from __future__ import annotations

import http.client
import unittest
from unittest import mock

from tools.checks import github_api


class GitHubApiTests(unittest.TestCase):
    def test_header_validation_error_never_includes_the_token(self) -> None:
        secret = "synthetic-secret\nunexpected"

        class RejectingOpener:
            def open(self, request, timeout):
                del timeout
                connection = http.client.HTTPConnection("example.invalid")
                connection.putrequest("GET", "/")
                connection.putheader("Authorization", request.get_header("Authorization"))

        errors: list[str] = []
        with mock.patch.object(
            github_api.urllib.request,
            "build_opener",
            return_value=RejectingOpener(),
        ):
            result = github_api.fetch_json(
                "https://api.github.com/repos/cadric/riteed/actions/runs/1",
                secret,
                errors,
                "run",
            )
        self.assertIsNone(result)
        self.assertTrue(errors)
        self.assertNotIn("synthetic-secret", "\n".join(errors))

    def test_terminal_page_may_have_only_prev_and_first_links(self) -> None:
        initial = "https://api.github.com/repos/cadric/riteed/actions/secrets?per_page=100"
        second = initial + "&page=2"
        responses = {
            initial: ({"total_count": 2, "secrets": [{"name": "A"}]}, f'<{second}>; rel="next"'),
            second: (
                {"total_count": 2, "secrets": [{"name": "B"}]},
                f'<{initial}>; rel="first", <{initial}>; rel="prev"',
            ),
        }
        errors: list[str] = []

        pages = github_api.fetch_pages(
            initial,
            "token",
            errors,
            "secrets",
            request_page=lambda url, _token: responses[url],
        )

        self.assertEqual(errors, [])
        self.assertEqual(len(pages or []), 2)

    def test_malformed_or_foreign_pagination_fails_closed(self) -> None:
        initial = "https://api.github.com/repos/cadric/riteed/actions/secrets?per_page=100"
        cases = (
            '<https://evil.example/steal?page=2>; rel="next"',
            '<https://api.github.com/repos/cadric/riteed/actions/secrets?per_page=100&page=2>; rel="next", broken',
            '<https://api.github.com:bad/repos/cadric/riteed/actions/secrets?page=2>; rel="next"',
        )
        for link in cases:
            with self.subTest(link=link):
                errors: list[str] = []
                pages = github_api.fetch_pages(
                    initial,
                    "token",
                    errors,
                    "secrets",
                    request_page=lambda _url, _token: ({"total_count": 0, "secrets": []}, link),
                )
                self.assertIsNone(pages)
                self.assertTrue(errors)

    def test_pagination_has_a_bounded_page_count(self) -> None:
        initial = "https://api.github.com/repos/cadric/riteed/actions/secrets?per_page=100"
        errors: list[str] = []

        def request(url: str, _token: str):
            page = int(url.rsplit("page=", 1)[1]) if "&page=" in url else 1
            next_url = initial + f"&page={page + 1}"
            return {"total_count": 1001, "secrets": []}, f'<{next_url}>; rel="next"'

        pages = github_api.fetch_pages(
            initial,
            "token",
            errors,
            "secrets",
            request_page=request,
        )
        self.assertIsNone(pages)
        self.assertTrue(any("page limit" in error for error in errors), errors)


if __name__ == "__main__":
    unittest.main()

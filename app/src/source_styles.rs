use std::sync::OnceLock;

const STYLE_RESOURCE_PATH: &str = "resource:///io/github/cadric/Riteed/styles";

static BUILTIN_STYLES_INSTALLED: OnceLock<()> = OnceLock::new();

pub(crate) fn install_builtin_style_schemes() {
    BUILTIN_STYLES_INSTALLED.get_or_init(|| {
        let manager = sourceview5::StyleSchemeManager::default();
        if !manager
            .search_path()
            .iter()
            .any(|path| path.as_str() == STYLE_RESOURCE_PATH)
        {
            manager.prepend_search_path(STYLE_RESOURCE_PATH);
            manager.force_rescan();
        }
    });
}

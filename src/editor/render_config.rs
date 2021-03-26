pub struct RenderConfig {
    pub tab_size: usize,
}

impl Default for RenderConfig {
    fn default() -> Self {
        RenderConfig { tab_size: 4 }
    }
}

use anyhow::Context;
use resvg::usvg::{Options, Tree};

pub(crate) fn is_svg_payload(bytes: &[u8]) -> bool {
    let sniff_len = bytes.len().min(512);
    let prefix = String::from_utf8_lossy(&bytes[..sniff_len]).to_lowercase();
    let trimmed = prefix.trim_start_matches(|ch: char| ch.is_whitespace() || ch == '\u{feff}');
    trimmed.starts_with("<svg")
        || (trimmed.starts_with("<?xml") && trimmed.contains("<svg"))
        || trimmed.contains("<svg")
}

pub(crate) fn parse_svg(bytes: &[u8], context: &str) -> anyhow::Result<Tree> {
    let mut options = Options::default();
    options.fontdb_mut().load_system_fonts();
    Tree::from_data(bytes, &options).with_context(|| context.to_owned())
}

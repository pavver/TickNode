const PREFIX: &str = "ticknode-plugin-api/1 ";
const SDK_VERSION: &str = env!("CARGO_PKG_VERSION");

#[doc(hidden)]
pub const PLUGIN_API_SECTION_LEN: usize =
    PREFIX.len() + ticknode_shared::PLUGIN_API.len() + 1 + SDK_VERSION.len();

#[doc(hidden)]
pub const PLUGIN_API_SECTION: [u8; PLUGIN_API_SECTION_LEN] = api_section();

const fn api_section() -> [u8; PLUGIN_API_SECTION_LEN] {
    let parts = [PREFIX, ticknode_shared::PLUGIN_API, " ", SDK_VERSION];
    let mut bytes = [0; PLUGIN_API_SECTION_LEN];
    let mut offset = 0;
    let mut part = 0;
    while part < parts.len() {
        let source = parts[part].as_bytes();
        let mut index = 0;
        while index < source.len() {
            bytes[offset] = source[index];
            offset += 1;
            index += 1;
        }
        part += 1;
    }
    bytes
}

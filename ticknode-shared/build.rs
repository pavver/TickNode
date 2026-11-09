use std::fs;

fn main() {
    let wit_path = "../wit/plugin.wit";
    let format_path = "plugin-value-format.json";
    println!("cargo:rerun-if-changed={wit_path}");
    println!("cargo:rerun-if-changed={format_path}");

    let wit = fs::read_to_string(wit_path).expect("read plugin WIT contract");
    let wit = wit
        .lines()
        .map(|line| line.split_once("//").map_or(line, |(code, _)| code))
        .collect::<Vec<_>>()
        .join("\n");
    let format = fs::read_to_string(format_path).expect("read plugin value format snapshot");
    let mut hash = 0xcbf29ce484222325_u64;
    for source in [&wit, &format] {
        let normalized = source.split_whitespace().collect::<Vec<_>>().join(" ");
        for byte in normalized.bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    println!("cargo:rustc-env=TICKNODE_PLUGIN_API={hash:016x}");
}

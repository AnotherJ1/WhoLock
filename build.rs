// 构建脚本：在 Windows 目标下编译并嵌入 manifest 资源。
//
// 使用 CARGO_CFG_TARGET_OS 环境变量判断 target（而非 cfg!，因为 build.rs 自身
// 是用 host triple 编译的，cfg! 反映的是 host 而非 target）。

fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    if target_os == "windows" {
        // 当 manifest 或 rc 文件改动时重新执行 build.rs
        println!("cargo:rerun-if-changed=resources/app.rc");
        println!("cargo:rerun-if-changed=resources/app.manifest");

        // embed-resource 2.x 签名：compile(resource_file, macros)
        // 不需要任何预处理器宏，因此传入 embed_resource::NONE。
        embed_resource::compile("resources/app.rc", embed_resource::NONE);
    }
}

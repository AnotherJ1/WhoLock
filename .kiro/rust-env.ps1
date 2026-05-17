# 本机 Rust 工具链路径（cargo / rustc / rustup）
# 由于该工具链未注册到系统 PATH，运行任何 cargo 命令前需 dot-source 本脚本：
#   . d:\work\cc\WhoLock\.kiro\rust-env.ps1
$env:CARGO_HOME = 'D:\software\rust\cargo'
$env:RUSTUP_HOME = 'D:\software\rust\rustup'
if (-not ($env:Path -split ';' -contains 'D:\software\rust\cargo\bin')) {
    $env:Path = 'D:\software\rust\cargo\bin;' + $env:Path
}

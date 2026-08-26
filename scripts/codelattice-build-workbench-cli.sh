#!/usr/bin/env bash
# codelattice-build-workbench-cli.sh —— 桌面工作台全语言 CLI 构建
#
# 用途：给桌面工作台 spawn 的 target/debug/codelattice 编入全部语言 feature，
# 使 `codelattice inspect` 各语言行报 analyzable: true（feature 维度）。
#
# 为何显式 feature 列表而非 --all-features：
# workspace 以后新增 optional feature 会被 --all-features 默默编进来，
# 显式列表让「工作台支持哪些语言」成为被评审的决策而非副作用。
# （shell 非 optional，无需列出。）
#
# 与 codelattice-precommit-check.sh 的关系：precommit 用默认 feature 跑测试，
# 本脚本只改 debug 二进制的 feature 集，不影响 precommit 及根 cargo test。
#
# vendor 风险处置：tree-sitter-cangjie / tree-sitter-arkts 依赖 vendor 语法源码，
# 本机编不过时从下方列表移除并在 docs/guides/workbench-cli-build.md
# 「构建不过」清单记录；禁止改 vendor / build.rs 绕过。
set -euo pipefail

cargo build -p gitnexus-rust-core-cli --features "\
tree-sitter-extraction,\
tree-sitter-typescript,\
tree-sitter-javascript,\
tree-sitter-python,\
tree-sitter-c,\
tree-sitter-cpp,\
tree-sitter-cangjie,\
tree-sitter-arkts"

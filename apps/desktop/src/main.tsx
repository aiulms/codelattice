// 入口：生产环境使用 Tauri transport；无 Tauri 环境（纯浏览器 dev）用 HTTP 兼容适配。
import React from "react";
import ReactDOM from "react-dom/client";
import { WorkbenchApp } from "./App";
import { TauriDesktopTransport } from "./transport/desktop-transport";
import { HttpDesktopTransport } from "./transport/http-transport";
import type { DesktopTransport } from "./types";
import { maybeRunSelftest } from "./smoke/selftest";
import { applyTheme, readTheme } from "./theme";
import "./styles.css";

applyTheme(readTheme());

function createTransport(): DesktopTransport {
  const isTauri = "__TAURI_INTERNALS__" in window;
  return isTauri ? new TauriDesktopTransport() : new HttpDesktopTransport("http://127.0.0.1:8765");
}

const transport = createTransport();
const root = ReactDOM.createRoot(document.getElementById("root") as HTMLElement);
root.render(
  <React.StrictMode>
    <WorkbenchApp transport={transport} />
  </React.StrictMode>,
);

void maybeRunSelftest(transport);

import React from "react";
import ReactDOM from "react-dom/client";
import "./styles/app.css";
import { DockWindow } from "./windows/DockWindow";
import { NotchWindow } from "./windows/NotchWindow";
import { SpotlightWindow } from "./windows/SpotlightWindow";
import { SettingsWindow } from "./windows/SettingsWindow";

const params = new URLSearchParams(window.location.search);
const view = params.get("view") ?? "dock";

function Root() {
  switch (view) {
    case "notch":
      return <NotchWindow />;
    case "spotlight":
      return <SpotlightWindow />;
    case "settings":
      return <SettingsWindow />;
    default:
      return <DockWindow />;
  }
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <Root />
  </React.StrictMode>
);

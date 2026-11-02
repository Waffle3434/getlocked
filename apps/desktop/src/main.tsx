import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import Popup from "./Popup";
import "./index.css";
import { applyCachedTheme } from "./theme";

// The block-message popup window loads index.html?popup (see src-tauri/src/popup.rs).
const isPopup = new URLSearchParams(location.search).has("popup");

applyCachedTheme();

createRoot(document.getElementById("root")!).render(<StrictMode>{isPopup ? <Popup /> : <App />}</StrictMode>);

import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App.jsx";
import { checkForUpdates } from "./updater.js";
import "./styles.css";

ReactDOM.createRoot(document.getElementById("root")).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);

// Non-blocking, after first paint. Never interrupts a running scan —
// the updater module checks app state before restarting.
setTimeout(checkForUpdates, 3000);

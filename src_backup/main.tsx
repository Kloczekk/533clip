import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { CaptureOverlay } from "./components/CaptureOverlay";
import { ErrorBoundary } from "./components/ErrorBoundary";
import "./styles/global.css";

const isOverlay = new URLSearchParams(window.location.search).has("overlay");

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <ErrorBoundary>{isOverlay ? <CaptureOverlay /> : <App />}</ErrorBoundary>
  </React.StrictMode>,
);

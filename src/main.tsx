import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { I18nProvider } from "./lib/i18n";
import { ThemeProvider } from "./lib/theme";
import { PrefsProvider } from "./lib/prefs";
import "./styles/index.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ThemeProvider>
      <I18nProvider>
        <PrefsProvider>
          <App />
        </PrefsProvider>
      </I18nProvider>
    </ThemeProvider>
  </React.StrictMode>
);

import React from "react";
import ReactDOM from "react-dom/client";
import { BrowserRouter, Routes, Route, Navigate } from "react-router-dom";
import { ThemeProvider } from "./lib/theme-provider";
import AudioPill from "./app/audio-pill/page";
import DashboardLayout from "./app/dashboard/layout";
import DashboardPage from "./app/dashboard/page";
import SettingsPage from "./app/settings/page";
import { UpdateChecker } from "./components/UpdateChecker";
import { MacRestartWarning, useMacRestartWarning } from "./components/MacRestartWarning";
import { Toaster } from "./components/ui/sonner";

function App() {
  const { shouldShow, isOpen, closeWarning } = useMacRestartWarning();

  return (
    <ThemeProvider defaultTheme="system" storageKey="vwisper-ui-theme">
      <UpdateChecker />
      {shouldShow && (
        <MacRestartWarning open={isOpen} onClose={closeWarning} />
      )}
      <BrowserRouter>
        <Routes>
          <Route path="/" element={<AudioPill />} />
          <Route path="/dashboard" element={
            <DashboardLayout>
              <DashboardPage />
            </DashboardLayout>
          } />
          <Route path="/settings" element={
            <DashboardLayout>
              <SettingsPage />
            </DashboardLayout>
          } />
          <Route path="*" element={<Navigate to="/dashboard" replace />} />
        </Routes>
      </BrowserRouter>
      <Toaster />
    </ThemeProvider>
  );
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);

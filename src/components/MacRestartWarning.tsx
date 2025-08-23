import { useState, useEffect } from "react";
import { AlertTriangle } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";

interface MacRestartWarningProps {
  open: boolean;
  onClose: () => void;
}

export function MacRestartWarning({ open, onClose }: MacRestartWarningProps) {
  const handleAcknowledge = async () => {
    try {
      // Store that the user has seen this warning in Tauri settings
      await invoke("mark_mac_restart_warning_shown");
    } catch (error) {
      console.error("Failed to save warning state to settings:", error);
    }
    
    // Also store in localStorage as a backup
    localStorage.setItem("vwisper-mac-restart-warning-seen", "true");
    onClose();
  };

  return (
    <Dialog open={open} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <div className="flex items-center gap-3">
            <AlertTriangle className="h-6 w-6 text-amber-500 flex-shrink-0" />
            <DialogTitle>macOS Permissions Required</DialogTitle>
          </div>
          <DialogDescription className="text-left mt-4 space-y-3">
            <p>
              <strong>VWisper requires special permissions</strong> to work with global hotkeys and text injection on macOS.
            </p>
            <p>
              When macOS prompts you to grant <strong>Accessibility</strong> and <strong>Microphone</strong> permissions, 
              please accept them. After granting these permissions, you'll need to <strong>completely restart VWisper</strong> 
              for the features to work properly.
            </p>
          </DialogDescription>
        </DialogHeader>
        
        <div className="bg-blue-50 dark:bg-blue-950/20 border border-blue-200 dark:border-blue-800 p-4 rounded-md">
          <div className="flex items-start gap-3">
            <div className="w-5 h-5 rounded-full bg-blue-500 text-white flex items-center justify-center text-xs font-bold flex-shrink-0 mt-0.5">
              i
            </div>
            <div className="text-sm">
              <p className="font-medium text-blue-900 dark:text-blue-100 mb-1">Why restart?</p>
              <p className="text-blue-800 dark:text-blue-200">
                macOS security requires applications to fully restart after accessibility permissions 
                are granted to properly access global keyboard events and system text injection.
              </p>
            </div>
          </div>
        </div>

        <DialogFooter className="gap-2">
          <Button onClick={handleAcknowledge} className="w-full">
            Got it, I'll restart after granting permissions
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

// Hook to check if we should show the Mac restart warning
export function useMacRestartWarning() {
  const [shouldShow, setShouldShow] = useState(false);
  const [isOpen, setIsOpen] = useState(false);

  useEffect(() => {
    const checkShouldShow = async () => {
      try {
        // Check if we're on macOS using Tauri
        const platform = await invoke<string>("get_platform");
        if (platform === "macos") {
          // Check settings first
          const settings = await invoke<any>("get_settings");
          if (!settings.mac_restart_warning_shown) {
            // Double-check with localStorage as backup
            const hasSeenWarning = localStorage.getItem("vwisper-mac-restart-warning-seen");
            if (!hasSeenWarning) {
              setShouldShow(true);
              setIsOpen(true);
            }
          }
        }
      } catch (error) {
        console.error("Failed to get platform or settings:", error);
        // Fallback to navigator.platform if Tauri calls fail
        if (navigator.platform.toLowerCase().includes('mac')) {
          const hasSeenWarning = localStorage.getItem("vwisper-mac-restart-warning-seen");
          if (!hasSeenWarning) {
            setShouldShow(true);
            setIsOpen(true);
          }
        }
      }
    };

    checkShouldShow();
  }, []);

  const closeWarning = () => {
    setIsOpen(false);
    setShouldShow(false);
  };

  return {
    shouldShow,
    isOpen,
    closeWarning,
  };
}
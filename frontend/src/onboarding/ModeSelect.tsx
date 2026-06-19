import { useState } from "react";

export type OnboardingMode = "client" | "host";

interface ModeSelectProps {
  /** Optional callback when a mode is selected. The onboarding shell can wire this. */
  onModeSelect?: (mode: OnboardingMode) => void;
}

export default function ModeSelect({ onModeSelect }: ModeSelectProps) {
  const [selectedMode, setSelectedMode] = useState<OnboardingMode | null>(null);

  const handleSelect = (mode: OnboardingMode) => {
    setSelectedMode(mode);
    onModeSelect?.(mode);
  };

  const handleContinue = () => {
    if (selectedMode) {
      onModeSelect?.(selectedMode);
    }
  };

  return (
    <div className="onboarding-mode-select">
      <h2 className="onboarding-title">Choose Your Setup Mode</h2>
      <p className="onboarding-description">
        Select how you want to use LoreGUI. You can connect to an existing server
        or set up your own.
      </p>

      <div className="onboarding-mode-cards">
        {/* Client Mode Card */}
        <div
          className={`onboarding-mode-card ${
            selectedMode === "client" ? "onboarding-mode-card--selected" : ""
          }`}
          onClick={() => handleSelect("client")}
          role="button"
          tabIndex={0}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              handleSelect("client");
            }
          }}
          aria-selected={selectedMode === "client"}
        >
          <div className="onboarding-mode-icon">
            <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
              <path d="M20 17.58A5 5 0 0 0 18 8h-1.26A8 8 0 1 0 4 16.25" />
              <line x1="8" y1="16" x2="8.01" y2="16" />
              <line x1="8" y1="20" x2="8.01" y2="20" />
              <line x1="12" y1="18" x2="12.01" y2="18" />
              <line x1="12" y1="22" x2="12.01" y2="22" />
              <line x1="16" y1="16" x2="16.01" y2="16" />
              <line x1="16" y1="20" x2="16.01" y2="20" />
            </svg>
          </div>
          <h3 className="onboarding-mode-title">Connect to a Lore Server</h3>
          <p className="onboarding-mode-description">
            Connect to an existing Lore server to collaborate with a team or access
            shared repositories.
          </p>
          <div className="onboarding-mode-features">
            <span>• Remote repository access</span>
            <span>• Team collaboration</span>
            <span>• Real-time sync</span>
          </div>
        </div>

        {/* Host Mode Card */}
        <div
          className={`onboarding-mode-card ${
            selectedMode === "host" ? "onboarding-mode-card--selected" : ""
          }`}
          onClick={() => handleSelect("host")}
          role="button"
          tabIndex={0}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              handleSelect("host");
            }
          }}
          aria-selected={selectedMode === "host"}
        >
          <div className="onboarding-mode-icon">
            <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
              <rect x="2" y="2" width="20" height="8" rx="2" ry="2" />
              <rect x="2" y="14" width="20" height="8" rx="2" ry="2" />
              <line x1="6" y1="6" x2="6.01" y2="6" />
              <line x1="6" y1="18" x2="6.01" y2="18" />
            </svg>
          </div>
          <h3 className="onboarding-mode-title">Set Up / Host a Server</h3>
          <p className="onboarding-mode-description">
            Create and host your own Lore server. Set up storage, repositories,
            and manage access for your team.
          </p>
          <div className="onboarding-mode-features">
            <span>• Full server control</span>
            <span>• Local storage options</span>
            <span>• Self-hosted privacy</span>
          </div>
        </div>
      </div>

      {selectedMode && (
        <div className="onboarding-mode-actions">
          <button
            className="onboarding-button onboarding-button--primary"
            onClick={handleContinue}
          >
            Continue with {selectedMode === "client" ? "Client" : "Host"} Mode
          </button>
        </div>
      )}
    </div>
  );
}

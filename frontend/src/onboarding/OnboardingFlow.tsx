import { useState } from "react";
import type { ReactNode } from "react";
import type { StorageBackendConfig } from "../api";
import ModeSelect, { type OnboardingMode } from "./ModeSelect";
import ClientConnect from "./ClientConnect";
import ClientClone from "./ClientClone";
import BackendPicker from "./server/BackendPicker";
import ValidateConnectivity from "./server/ValidateConnectivity";
import InitStore, { type InitStoreResult } from "./server/InitStore";
import ServiceSetup from "./server/ServiceSetup";

interface OnboardingFlowProps {
  /** Called once the user finishes (or skips to the end of) the chosen flow. */
  onComplete: () => void;
}

const HOST_STEPS = ["backend", "validate", "init", "service"] as const;
const CLIENT_STEPS = ["connect", "clone"] as const;

const STEP_LABELS: Record<string, string> = {
  backend: "Storage backend",
  validate: "Validate connectivity",
  init: "Initialize server",
  service: "Host server",
  connect: "Connect to server",
  clone: "Clone / open repository",
};

/**
 * First-run onboarding shell. Wires the per-step components
 * (ModeSelect → client|host flow) into a single guided stepper.
 *
 * The individual step components own their own success/error state and
 * backend calls; this shell only sequences them and forwards the storage
 * config from the backend picker to the connectivity check.
 */
export default function OnboardingFlow({ onComplete }: OnboardingFlowProps) {
  const [mode, setMode] = useState<OnboardingMode | null>(null);
  const [stepIndex, setStepIndex] = useState(0);
  const [backendConfig, setBackendConfig] =
    useState<StorageBackendConfig | null>(null);
  const [initResult, setInitResult] = useState<InitStoreResult | null>(null);

  const steps: readonly string[] =
    mode === "host" ? HOST_STEPS : mode === "client" ? CLIENT_STEPS : [];
  const current = steps[stepIndex];

  const next = () => {
    if (stepIndex + 1 >= steps.length) {
      onComplete();
    } else {
      setStepIndex((i) => i + 1);
    }
  };

  const back = () => {
    if (stepIndex === 0) {
      setMode(null);
    } else {
      setStepIndex((i) => i - 1);
    }
  };

  if (!mode) {
    return (
      <div className="onboarding">
        <ModeSelect
          onModeSelect={(m) => {
            setMode(m);
            setStepIndex(0);
          }}
        />
      </div>
    );
  }

  let content: ReactNode = null;
  if (mode === "client") {
    content = current === "connect" ? <ClientConnect /> : <ClientClone />;
  } else {
    switch (current) {
      case "backend":
        content = <BackendPicker onConfigured={setBackendConfig} />;
        break;
      case "validate":
        content = backendConfig ? (
          <ValidateConnectivity config={backendConfig} />
        ) : (
          <div className="onboarding-card">
            <h2>Validate Backend Connectivity</h2>
            <p className="onboarding-description">
              Configure and open a storage backend on the previous step before
              running the connectivity test.
            </p>
          </div>
        );
        break;
      case "init":
        content = <InitStore onInitialized={setInitResult} />;
        break;
      case "service":
        content = (
          <ServiceSetup
            storePath={initResult?.storePath}
            repoName={initResult?.repoName}
          />
        );
        break;
    }
  }

  const isLast = stepIndex + 1 >= steps.length;

  return (
    <div className="onboarding">
      <div className="onboarding-stepper">
        {steps.map((s, i) => (
          <span
            key={s}
            className={`onboarding-step ${
              i === stepIndex
                ? "onboarding-step--active"
                : i < stepIndex
                  ? "onboarding-step--done"
                  : ""
            }`}
          >
            {i + 1}. {STEP_LABELS[s] ?? s}
          </span>
        ))}
      </div>

      {content}

      <div className="onboarding-nav">
        <button className="onboarding-button" onClick={back}>
          Back
        </button>
        <button
          className="onboarding-button onboarding-button--primary"
          onClick={next}
        >
          {isLast ? "Finish" : "Continue"}
        </button>
      </div>
    </div>
  );
}

import { useCallback, useRef, useState } from "react";
import type { ReactNode } from "react";
import type { StorageBackendConfig } from "../api";
import ModeSelect, { type OnboardingMode } from "./ModeSelect";
import ClientConnect from "./ClientConnect";
import ClientClone, { type ClientRepositoryMode } from "./ClientClone";
import BackendPicker from "./server/BackendPicker";
import ValidateConnectivity from "./server/ValidateConnectivity";
import InitStore, { type InitStoreResult } from "./server/InitStore";
import ServiceSetup from "./server/ServiceSetup";
import type { StepResult } from "./stepResult";

export type { StepResult, StepStatus } from "./stepResult";

interface OnboardingFlowProps {
  /** Called once the user finishes (or skips to the end of) the chosen flow. */
  onComplete: () => void;
  /** Optional guided-hub destination. "setup" retains the normal mode chooser. */
  initialIntent?: OnboardingIntent;
}

export type OnboardingIntent =
  | "setup"
  | "connect"
  | "host"
  | "open"
  | "create";

const HOST_STEPS = ["backend", "validate", "init", "service"] as const;
const CLIENT_STEPS = ["connect", "clone"] as const;
type FlowStep = (typeof HOST_STEPS)[number] | (typeof CLIENT_STEPS)[number];

export type HostNextAction =
  | "browse-repositories"
  | "create-repository"
  | "open-existing"
  | "manage-server-only";

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
 * The individual step components own their backend calls and visible errors,
 * while this shell owns the reported result for each step. Navigation is
 * therefore based on backend success rather than on a step merely rendering.
 */
function initialRoute(intent: OnboardingIntent): {
  mode: OnboardingMode | null;
  stepIndex: number;
  repositoryMode: ClientRepositoryMode;
} {
  switch (intent) {
    case "connect":
      return { mode: "client", stepIndex: 0, repositoryMode: "choice" };
    case "host":
      return { mode: "host", stepIndex: 0, repositoryMode: "choice" };
    case "open":
      return { mode: "client", stepIndex: 1, repositoryMode: "open" };
    case "create":
      return { mode: "client", stepIndex: 1, repositoryMode: "create" };
    default:
      return { mode: null, stepIndex: 0, repositoryMode: "choice" };
  }
}

export default function OnboardingFlow({
  onComplete,
  initialIntent = "setup",
}: OnboardingFlowProps) {
  const route = initialRoute(initialIntent);
  const [mode, setMode] = useState<OnboardingMode | null>(route.mode);
  const [stepIndex, setStepIndex] = useState(route.stepIndex);
  const [backendConfig, setBackendConfig] =
    useState<StorageBackendConfig | null>(null);
  const [initResult, setInitResult] = useState<InitStoreResult | null>(null);
  const [stepResults, setStepResults] = useState<
    Partial<Record<FlowStep, StepResult<unknown>>>
  >({});
  const [hostNextAction, setHostNextAction] =
    useState<HostNextAction | null>(null);
  const [hostRepositoryResult, setHostRepositoryResult] =
    useState<StepResult<string>>({ status: "idle" });
  const routeEpochRef = useRef(0);

  const steps: readonly string[] =
    mode === "host" ? HOST_STEPS : mode === "client" ? CLIENT_STEPS : [];
  const current = steps[stepIndex];
  const renderedRouteEpoch = routeEpochRef.current;

  const reportStep = useCallback(
    <T,>(step: FlowStep, result: StepResult<T>) => {
      setStepResults((previous) => {
        const nextResults = { ...previous, [step]: result };
        const stepPosition =
          mode === "host"
            ? HOST_STEPS.indexOf(step as (typeof HOST_STEPS)[number])
            : CLIENT_STEPS.indexOf(step as (typeof CLIENT_STEPS)[number]);
        if (result.status !== "success" && stepPosition >= 0) {
          const activeSteps = mode === "host" ? HOST_STEPS : CLIENT_STEPS;
          for (const downstream of activeSteps.slice(stepPosition + 1)) {
            delete nextResults[downstream];
          }
        }
        return nextResults;
      });
      if (step === "service" && result.status !== "success") {
        setHostNextAction(null);
        setHostRepositoryResult({ status: "idle" });
      }
    },
    [mode],
  );

  const reportBackend = useCallback(
    (result: StepResult<StorageBackendConfig>) => {
      if (routeEpochRef.current !== renderedRouteEpoch) return;
      reportStep("backend", result);
    },
    [reportStep, renderedRouteEpoch],
  );
  const reportValidate = useCallback(
    (result: StepResult) => {
      if (routeEpochRef.current !== renderedRouteEpoch) return;
      reportStep("validate", result);
    },
    [reportStep, renderedRouteEpoch],
  );
  const reportInit = useCallback(
    (result: StepResult<InitStoreResult>) => {
      if (routeEpochRef.current !== renderedRouteEpoch) return;
      reportStep("init", result);
    },
    [reportStep, renderedRouteEpoch],
  );
  const reportService = useCallback(
    (result: StepResult<string>) => {
      if (routeEpochRef.current !== renderedRouteEpoch) return;
      reportStep("service", result);
    },
    [reportStep, renderedRouteEpoch],
  );
  const reportConnect = useCallback(
    (result: StepResult<string>) => {
      if (routeEpochRef.current !== renderedRouteEpoch) return;
      reportStep("connect", result);
    },
    [reportStep, renderedRouteEpoch],
  );
  const reportClone = useCallback(
    (result: StepResult<string>) => {
      if (routeEpochRef.current !== renderedRouteEpoch) return;
      reportStep("clone", result);
    },
    [reportStep, renderedRouteEpoch],
  );
  const acceptBackendConfig = useCallback(
    (config: StorageBackendConfig) => {
      if (routeEpochRef.current !== renderedRouteEpoch) return;
      setBackendConfig(config);
    },
    [renderedRouteEpoch],
  );
  const acceptInitResult = useCallback(
    (result: InitStoreResult) => {
      if (routeEpochRef.current !== renderedRouteEpoch) return;
      setInitResult(result);
    },
    [renderedRouteEpoch],
  );
  const reportHostRepository = useCallback(
    (result: StepResult<string>) => {
      if (routeEpochRef.current !== renderedRouteEpoch) return;
      setHostRepositoryResult(result);
    },
    [renderedRouteEpoch],
  );

  const currentResult = current
    ? stepResults[current as FlowStep] ?? { status: "idle" as const }
    : { status: "idle" as const };
  const isLast = stepIndex + 1 >= steps.length;
  const hostFinishReady =
    mode === "host" &&
    current === "service" &&
    currentResult.status === "success" &&
    (hostNextAction === "manage-server-only" ||
      (hostNextAction !== null &&
        hostRepositoryResult.status === "success"));
  const navigationReady = isLast
    ? mode === "host"
      ? hostFinishReady
      : currentResult.status === "success"
    : currentResult.status === "success";

  const next = () => {
    if (!navigationReady) return;
    routeEpochRef.current += 1;
    if (stepIndex + 1 >= steps.length) {
      onComplete();
    } else {
      setStepIndex((i) => i + 1);
    }
  };

  const back = () => {
    routeEpochRef.current += 1;
    if (
      mode === "host" &&
      current === "service" &&
      hostNextAction &&
      hostNextAction !== "manage-server-only"
    ) {
      setHostNextAction(null);
      setHostRepositoryResult({ status: "idle" });
      return;
    }
    if (stepIndex === 0) {
      setMode(null);
      setStepResults({});
      setBackendConfig(null);
      setInitResult(null);
      setHostNextAction(null);
      setHostRepositoryResult({ status: "idle" });
    } else {
      const destinationIndex = stepIndex - 1;
      setStepResults((previous) => {
        const kept = { ...previous };
        for (const downstream of steps.slice(destinationIndex + 1)) {
          delete kept[downstream as FlowStep];
        }
        return kept;
      });
      setHostNextAction(null);
      setHostRepositoryResult({ status: "idle" });
      setStepIndex((i) => i - 1);
    }
  };

  if (!mode) {
    return (
      <div className="onboarding">
        <ModeSelect
          onModeSelect={(m) => {
            routeEpochRef.current += 1;
            setMode(m);
            setStepIndex(0);
            setStepResults({});
            setBackendConfig(null);
            setInitResult(null);
            setHostNextAction(null);
            setHostRepositoryResult({ status: "idle" });
          }}
        />
      </div>
    );
  }

  let content: ReactNode = null;
  if (mode === "client") {
    content =
      current === "connect" ? (
        <ClientConnect onStateChange={reportConnect} />
      ) : (
        <ClientClone
          initialMode={
            stepResults.connect?.status === "success"
              ? "clone"
              : route.repositoryMode
          }
          initialCloneUrl={
            stepResults.connect?.status === "success"
              ? (stepResults.connect.value as string)
              : undefined
          }
          onStateChange={reportClone}
        />
      );
  } else {
    switch (current) {
      case "backend":
        content = (
          <BackendPicker
            onConfigured={acceptBackendConfig}
            onStateChange={reportBackend}
          />
        );
        break;
      case "validate":
        content = backendConfig ? (
          <ValidateConnectivity
            config={backendConfig}
            onStateChange={reportValidate}
          />
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
        content = (
          <InitStore
            config={backendConfig ?? undefined}
            onInitialized={acceptInitResult}
            onStateChange={reportInit}
          />
        );
        break;
      case "service":
        content = (
          <ServiceSetup
            storePath={initResult?.storePath}
            repoName={initResult?.repoName}
            onStateChange={reportService}
          />
        );
        break;
    }
  }

  const hostServiceUrl =
    stepResults.service?.status === "success"
      ? (stepResults.service.value as string)
      : undefined;
  const repositoryModeForHost =
    hostNextAction === "browse-repositories"
      ? "clone"
      : hostNextAction === "create-repository"
        ? "create"
        : "open";

  const blockedTitle =
    mode === "host" &&
    current === "service" &&
    hostNextAction &&
    hostNextAction !== "manage-server-only" &&
    hostRepositoryResult.status !== "success"
      ? hostRepositoryResult.status === "working"
        ? "Wait for the repository operation to finish."
        : hostRepositoryResult.message ??
          "Open, create, or clone a local repository before finishing."
      : currentResult.status === "working"
      ? "Wait for this step to finish."
      : currentResult.status === "error"
        ? currentResult.message ?? "Resolve this step's error before continuing."
        : isLast && mode === "host" && currentResult.status === "success"
          ? "Choose what to do after hosting before finishing."
          : "Complete this step successfully before continuing.";

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

      {mode === "host" &&
        current === "service" &&
        currentResult.status === "success" && (
          <div className="onboarding-card" aria-label="After hosting">
            <h2>What would you like to do next?</h2>
            <div className="onboarding-radio-group">
              {(
                [
                  ["browse-repositories", "Browse repositories"],
                  ["create-repository", "Create repository"],
                  ["open-existing", "Open existing"],
                  ["manage-server-only", "Manage server only"],
                ] as const
              ).map(([value, label]) => (
                <label className="onboarding-radio" key={value}>
                  <input
                    type="radio"
                    name="host-next-action"
                    value={value}
                    checked={hostNextAction === value}
                    onChange={() => {
                      routeEpochRef.current += 1;
                      setHostNextAction(value);
                      setHostRepositoryResult({ status: "idle" });
                    }}
                  />
                  <span className="onboarding-radio-label">{label}</span>
                </label>
              ))}
            </div>
            {hostNextAction === "manage-server-only" && (
              <p className="onboarding-description">
                Repository actions will remain unavailable until you open,
                create, or clone a local repository.
              </p>
            )}
            {hostNextAction && hostNextAction !== "manage-server-only" && (
              <ClientClone
                key={hostNextAction}
                initialMode={repositoryModeForHost}
                initialCloneUrl={
                  hostNextAction === "browse-repositories"
                    ? hostServiceUrl
                    : undefined
                }
                onStateChange={reportHostRepository}
              />
            )}
          </div>
        )}

      <div className="onboarding-nav">
        <button className="onboarding-button" onClick={back}>
          Back
        </button>
        <button
          className="onboarding-button onboarding-button--primary"
          onClick={next}
          disabled={!navigationReady}
          title={navigationReady ? undefined : blockedTitle}
        >
          {isLast ? "Finish" : "Continue"}
        </button>
      </div>
    </div>
  );
}

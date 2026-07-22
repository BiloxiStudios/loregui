export type StepStatus = "idle" | "working" | "success" | "error";

export interface StepResult<T = void> {
  status: StepStatus;
  value?: T;
  message?: string;
}

export interface StepStateProps<T = void> {
  onStateChange?: (result: StepResult<T>) => void;
}

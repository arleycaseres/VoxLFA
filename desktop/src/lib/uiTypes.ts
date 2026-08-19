export type UiSeverity = "critical" | "recommended" | "optional";

export type DetectedField = {
  label: string;
  value?: number;
  unit?: string;
};

export type RecommendationField = {
  label: string;
  payload?: any;
};

export type UiAction = {
  type: string;
  payload?: any;
} | null;

export type UiSuggestion = {
  id: number;
  kind: string;
  detected: DetectedField;
  consequence: string;
  recommendation: RecommendationField;
  severity: UiSeverity;
  action?: UiAction;
};

export default UiSuggestion;

import { useEffect, useState } from "react";
import { desktop, type AnalyticsRange, type AnalyticsResponse } from "./desktop";

export interface AnalyticsState {
  analytics: AnalyticsResponse | null;
  loading: boolean;
  error: string;
  reload: () => void;
}

/**
 * Fetches local analytics for a range. `active=false` (browser preview or
 * hidden page) performs no request. Local SQLite reads are cheap and bounded.
 */
export function useAnalytics(
  native: boolean,
  range: AnalyticsRange,
  active = true,
): AnalyticsState {
  const [analytics, setAnalytics] = useState<AnalyticsResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [reloadKey, setReloadKey] = useState(0);

  useEffect(() => {
    if (!native || !active) {
      setLoading(false);
      return;
    }
    let alive = true;
    setLoading(true);
    setError("");
    desktop
      .getAnalytics(range)
      .then((result) => {
        if (alive) setAnalytics(result);
      })
      .catch(() => {
        if (alive) setError("Ellie could not load local history.");
      })
      .finally(() => {
        if (alive) setLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [native, range, reloadKey, active]);

  return {
    analytics,
    loading,
    error,
    reload: () => setReloadKey((value) => value + 1),
  };
}
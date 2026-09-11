import type { ProviderOverview } from "./desktop";

export interface MiniQuotaMetric {
  providerId: string;
  providerName: string;
  windowId: string;
  windowLabel: string;
  remainingPercent: number;
  stale: boolean;
  lastSuccessfulRefresh: string | null;
}

export function selectMiniQuotaMetrics(
  providers: ProviderOverview[],
  hiddenProviderIds: string[],
): MiniQuotaMetric[] {
  const hidden = new Set(hiddenProviderIds);
  return providers.flatMap((provider) => {
    const snapshot = provider.snapshot;
    if (
      hidden.has(provider.providerId) ||
      !snapshot ||
      snapshot.dataKind !== "live" ||
      snapshot.hasSubscription === false ||
      !snapshot.capabilities.quotaWindows
    ) {
      return [];
    }
    return snapshot.windows.flatMap((window) => {
      if (
        window.source !== "provider_reported" ||
        window.remainingPercent === null ||
        !Number.isFinite(window.remainingPercent) ||
        window.remainingPercent < 0 ||
        window.remainingPercent > 100
      ) {
        return [];
      }
      return [{
        providerId: provider.providerId,
        providerName: provider.displayName,
        windowId: window.id,
        windowLabel: window.label,
        remainingPercent: window.remainingPercent,
        stale: provider.stale === true,
        lastSuccessfulRefresh:
          provider.lastSuccessfulRefresh ?? snapshot.fetchedAt,
      }];
    });
  });
}

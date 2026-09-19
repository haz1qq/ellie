/** Calendar period chosen for the GitHub profile contribution card. */
export type ContributionYear = "rolling" | string;

export interface ContributionYearOption {
  value: ContributionYear;
  label: string;
}

/** Maps the visible period selection to the typed IPC query. */
export function contributionQueryOf(year: ContributionYear): { year?: number } {
  return year === "rolling" ? {} : { year: Number(year) };
}

export function yearOptions(): ContributionYearOption[] {
  const currentYear = new Date().getFullYear();
  const options: ContributionYearOption[] = [
    { value: "rolling", label: "Last 12 months" },
  ];
  for (let year = currentYear; year >= Math.max(2008, currentYear - 5); year -= 1) {
    options.push({ value: String(year), label: String(year) });
  }
  return options;
}
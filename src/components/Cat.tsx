export function Cat({ small = false }: { small?: boolean }) {
  return (
    <svg
      className={small ? "cat cat-small" : "cat"}
      viewBox="0 0 160 142"
      aria-hidden="true"
      focusable="false"
    >
      <path
        d="M118 124c37 0 36-39 18-32"
        fill="none"
        stroke="#f2eee8"
        strokeWidth="13"
        strokeLinecap="round"
      />
      <path d="M44 125c-1-30 6-53 35-53 31 0 40 23 39 53" fill="#f2eee8" />
      <path d="M87 84c17 1 29 16 31 41H83c-7-15-5-30 4-41" fill="#232326" />
      <path
        d="M38 62 35 15 66 34c9-3 19-3 28 0l30-19-1 48c4 32-18 44-43 44S35 93 38 62"
        fill="#f2eee8"
        stroke="#777479"
        strokeWidth="2"
        strokeLinejoin="round"
      />
      <path d="m37 18 29 17c-1 26-10 39-28 40-3-13-1-30-1-57" fill="#232326" />
      <path d="m44 30 3 21 11-12zm70 0-13 9 11 12z" fill="#d7a4b3" />
      <path
        d="M51 66q7-7 14 0m31 0q7-7 14 0"
        fill="none"
        stroke="#777479"
        strokeWidth="3"
        strokeLinecap="round"
      />
      <path d="m75 77 10 0-5 6z" fill="#d7a4b3" />
      <path
        d="M80 83v4m0 0q-5 7-10 0m10 0q5 7 10 0"
        fill="none"
        stroke="#232326"
        strokeWidth="2"
        strokeLinecap="round"
      />
      <path
        d="m48 81-20-3m20 10-19 3m83-10 20-3m-20 10 19 3"
        stroke="#999498"
        strokeWidth="2"
        strokeLinecap="round"
      />
      <path
        d="M47 126h25m18 0h25"
        stroke="#f2eee8"
        strokeWidth="13"
        strokeLinecap="round"
      />
    </svg>
  );
}

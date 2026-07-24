// Magma's mark: a stylized flame. Inline SVG so it stays crisp at any size and
// needs no external asset. Reused by the sidebar logo, splash, and About panel.
export default function FlameIcon({ size = 24 }: { size?: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 48 48"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      aria-hidden="true"
    >
      <defs>
        <linearGradient id="magma-flame-outer" x1="24" y1="4" x2="24" y2="44" gradientUnits="userSpaceOnUse">
          <stop stopColor="#ff963d" />
          <stop offset="1" stopColor="#e0533d" />
        </linearGradient>
        <linearGradient id="magma-flame-inner" x1="24" y1="18" x2="24" y2="42" gradientUnits="userSpaceOnUse">
          <stop stopColor="#fff0be" />
          <stop offset="1" stopColor="#ffb45a" />
        </linearGradient>
      </defs>
      <path
        d="M24 3c2 7-4 10-7 15-2.6 4.3-3 8-.8 3.2C17 18 19 19 19 22c0 2.4-1.8 3.6-1.8 6.4C17.2 37 20.9 44 24 44s10-4.7 10-13c0-6.4-3.7-9.6-4.8-14.8-1-4.6 1.2-7 -1.2-11C26.6 2 24.8 1.2 24 3Z"
        fill="url(#magma-flame-outer)"
      />
      <path
        d="M24.6 22c1.2 3 4 4.4 4 8.2 0 4.2-2.4 7.8-5 7.8-2.4 0-4.8-2.7-4.8-6.2 0-3.5 2.3-4.6 3-7.6.5-2.2-.2-4 1-4 .9 0 1.3.9 1.8 1.8Z"
        fill="url(#magma-flame-inner)"
      />
    </svg>
  );
}

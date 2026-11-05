export default function Logo({ size = 28 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 28 28" aria-hidden="true">
      <rect x="5" y="18" width="18" height="6" rx="2" fill="#34D399" />
      <rect x="8" y="11" width="12" height="6" rx="2" fill="#6EE7B7" />
      <rect x="10.5" y="4" width="7" height="6" rx="2" fill="#A7F3D0" />
    </svg>
  );
}

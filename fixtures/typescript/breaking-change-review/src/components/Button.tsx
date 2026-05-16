export function Button({ label, onClick }: { label: string; onClick: () => void }) {
  return { type: "button", label, onClick };
}

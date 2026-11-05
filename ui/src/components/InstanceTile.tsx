import { initial, tileColor } from "../lib/instances";

export default function InstanceTile({ id, name, size = 46 }: { id: string; name: string; size?: number }) {
  return (
    <div
      aria-hidden="true"
      className="grid flex-none place-items-center rounded-[10px] font-mono text-[#101217]"
      style={{ width: size, height: size, background: tileColor(id), fontSize: size * 0.43 }}
    >
      {initial(name)}
    </div>
  );
}

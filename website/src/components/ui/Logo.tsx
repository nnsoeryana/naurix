import Image from "next/image";

export function Logo({ className = "" }: { className?: string }) {
  return (
    <span className={`inline-flex items-center gap-2.5 ${className}`}>
      <Image src="/naurix-mark.svg" alt="" width={34} height={18} className="h-[18px] w-auto" />
      <span className="text-[17px] font-semibold tracking-tight">Naurix</span>
    </span>
  );
}

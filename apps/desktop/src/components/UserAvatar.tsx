import { useEffect, useState } from "react";
import { fetchSharedAvatar, onSharedAvatarChanged } from "../lib/sharedApi";

export function userInitials(name: string): string {
  return name.split(/\s+/).filter(Boolean).slice(0, 2).map((part) => Array.from(part)[0]).join("").toUpperCase() || "U";
}

/** A person's profile picture, or their initials when they have none. Decorative: the name is always shown next to it. */
export function UserAvatar({ className = "", name, size = 36, userId }: { className?: string; name: string; size?: number; userId: string }) {
  const [url, setUrl] = useState<string | null>(null);
  const [version, setVersion] = useState(0);

  useEffect(() => onSharedAvatarChanged((changed) => { if (changed === userId) setVersion((current) => current + 1); }), [userId]);

  useEffect(() => {
    let active = true;
    setUrl(null);
    void fetchSharedAvatar(userId).then((next) => { if (active) setUrl(next); });
    return () => { active = false; };
  }, [userId, version]);

  return <span aria-hidden="true" className={`user-avatar ${className}`.trim()} style={{ width: size, height: size, fontSize: Math.round(size * 0.38) }}>
    {url ? <img alt="" draggable={false} onError={() => setUrl(null)} src={url} /> : userInitials(name)}
  </span>;
}

import { IconAlertTriangle as Alert, IconFileCheck as File } from "@tabler/icons-react";
import { AuthFrame } from "./AuthFrame";
import { ICON_SIZE } from "./ui/icon";

/**
 * Shown instead of the connect screen when the administrator's configuration file cannot be used (V0.4.47). The file is named and
 * nothing in it is obeyed; the person is not asked for an address, because that would go around the file.
 */
export function DesktopConfigProblem({ problem }: { problem: string }) {
  return (
    <AuthFrame
      copy="The app found the configuration file your administrator put with it, but cannot use it, so it has not connected to any server."
      eyebrow="Desktop app"
      formLabel="The configuration file"
      introLabel="RepoMemo desktop"
      title="This app's configuration cannot be used."
      trust={[
        { icon: <File size={ICON_SIZE.md} />, title: "Nothing was changed", text: "Your account and everything on the server are as you left them." },
        { icon: <Alert size={ICON_SIZE.md} />, title: "Nothing in the file is used", text: "Not even the part that is right: the file is used whole or not at all." },
      ]}
    >
      <div className="shared-auth-heading">
        <p className="shared-eyebrow">Configuration</p>
        <h2>Ask your administrator to correct it</h2>
        <p>Then start the app again. This is what is wrong:</p>
      </div>
      <p className="shared-auth-hint" role="alert"><code>{problem}</code></p>
    </AuthFrame>
  );
}

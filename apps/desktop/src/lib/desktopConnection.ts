import { configureDesktopApi } from "./sharedApi";
import { shellTransport } from "./transport";

/**
 * Points the client at a server the shell has allowed (V0.2.49): its requests go out through the shell's transport, which calls that
 * server and no other and follows no redirect.
 */
export async function pointAtServer(origin: string): Promise<void> {
  configureDesktopApi(origin, await shellTransport());
}

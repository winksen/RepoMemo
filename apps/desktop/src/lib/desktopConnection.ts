import { desktopServer } from "./desktop";
import { configureDesktopApi } from "./sharedApi";
import { shellTransport } from "./transport";

/**
 * Points the client at a server the shell has allowed (V0.2.49): its requests go out through the shell's transport, which calls that
 * server and no other and follows no redirect. A server with a private certificate authority (V0.4.55) is reached through the
 * shell's relay, which the shell names; the page still knows the server by its own address.
 */
export async function pointAtServer(origin: string): Promise<void> {
  const route = await Promise.resolve().then(() => desktopServer.route()).catch(() => null);
  configureDesktopApi(origin, await shellTransport(route ? { origin, proxy: route.proxy, token: route.token } : undefined));
}

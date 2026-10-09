//! What the admin console shows on every surface, so the terminal client
//! (`repomemo-console`) and System › Console in the web app look the same.

/// The logo printed when a console opens: 72 columns, box-drawing characters.
pub const CONSOLE_BANNER: &str = concat!(
    "██████╗ ███████╗██████╗  ██████╗ ███╗   ███╗███████╗███╗   ███╗ ██████╗\n",
    "██╔══██╗██╔════╝██╔══██╗██╔═══██╗████╗ ████║██╔════╝████╗ ████║██╔═══██╗\n",
    "██████╔╝█████╗  ██████╔╝██║   ██║██╔████╔██║█████╗  ██╔████╔██║██║   ██║\n",
    "██╔══██╗██╔══╝  ██╔═══╝ ██║   ██║██║╚██╔╝██║██╔══╝  ██║╚██╔╝██║██║   ██║\n",
    "██║  ██║███████╗██║     ╚██████╔╝██║ ╚═╝ ██║███████╗██║ ╚═╝ ██║╚██████╔╝\n",
    "╚═╝  ╚═╝╚══════╝╚═╝      ╚═════╝ ╚═╝     ╚═╝╚══════╝╚═╝     ╚═╝ ╚═════╝\n",
);

/// The line under the logo.
pub const CONSOLE_TAGLINE: &str = "Team memory for technical knowledge · admin console";

/// What a new console session suggests first.
pub const CONSOLE_TIPS: &str = "Type help to list the commands. Changes are dry runs until you add --yes; add --json to see the raw answer.";

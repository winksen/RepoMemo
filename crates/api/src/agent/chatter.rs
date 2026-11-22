//! What CAIRN does with a message that asks for no work (V0.9.1): a greeting, a thank-you, "who are you", "what can you do".
//! These are recognised by whole phrases and answered at once from CAIRN's own description, with no AI and no search, so they
//! work without a provider and never fall into Ask, where the model would look for the answer in the indexed files.

use super::{agent_capabilities, reply};
use repomemo_domain::{AgentCapability, AgentReply, AgentRouting};

/// What a chatty message is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChatterKind {
    /// Only a greeting.
    Greeting,
    HowAreYou,
    Identity,
    Capabilities,
    Thanks,
    Farewell,
}

/// A recognised chatty message: what it is, and the greeting it opened with, so CAIRN can greet back the same way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Chatter {
    pub kind: ChatterKind,
    pub greeting: Option<&'static str>,
}

/// The workspace CAIRN is working in, as a greeting names it (V0.9.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkspaceBrief {
    pub name: String,
    pub files: i64,
}

/// What a reply may say about the workspace and CAIRN's means.
#[derive(Debug, Clone, Default)]
pub(crate) struct ChatterContext {
    pub ai_available: bool,
    pub workspace: Option<WorkspaceBrief>,
    /// Questions the workspace's template offers as buttons (V0.9.7), shown as ideas of what to ask.
    pub suggestions: Vec<String>,
}

/// How many of a template's questions a reply shows.
const IDEAS_SHOWN: usize = 3;

impl ChatterContext {
    /// The template's questions as a short Markdown list to follow the jobs, or nothing when it offers none.
    fn ideas_markdown(&self) -> String {
        let ideas = self
            .suggestions
            .iter()
            .filter(|idea| !idea.trim().is_empty())
            .take(IDEAS_SHOWN)
            .map(|idea| format!("- {}", idea.trim()))
            .collect::<Vec<_>>();
        if ideas.is_empty() {
            String::new()
        } else {
            format!("\n\nThis workspace is set up for questions like:\n{}", ideas.join("\n"))
        }
    }

    /// How the workspace is named in a sentence: by name with what is indexed, or just "this workspace".
    fn workspace_clause(&self) -> String {
        match &self.workspace {
            Some(workspace) if workspace.files > 0 => format!(
                "**{}** ({} indexed {})",
                workspace.name,
                workspace.files,
                if workspace.files == 1 { "file" } else { "files" }
            ),
            Some(workspace) => format!("**{}** (nothing is indexed yet)", workspace.name),
            None => "this workspace".to_owned(),
        }
    }
}

/// Greetings by their words, longest first, and the word CAIRN answers with.
const GREETINGS: &[(&[&str], &str)] = &[
    (&["ahlan", "wa", "sahlan"], "Ahlan"),
    (&["assalamu", "alaikum"], "Salam"),
    (&["assalamu", "alaykum"], "Salam"),
    (&["assalamu", "alaikoum"], "Salam"),
    (&["as", "salamu", "alaykum"], "Salam"),
    (&["as", "salamu", "alaikum"], "Salam"),
    (&["salam", "alaykum"], "Salam"),
    (&["salam", "alaikum"], "Salam"),
    (&["salam", "alaikoum"], "Salam"),
    (&["good", "morning"], "Good morning"),
    (&["good", "afternoon"], "Good afternoon"),
    (&["good", "evening"], "Good evening"),
    (&["buenos", "dias"], "Hola"),
    (&["hello", "there"], "Hello"),
    (&["hi", "there"], "Hello"),
    (&["hey", "there"], "Hello"),
    (&["salam"], "Salam"),
    (&["salaam"], "Salam"),
    (&["slm"], "Salam"),
    (&["marhaba"], "Marhaba"),
    (&["ahlan"], "Ahlan"),
    (&["bonjour"], "Bonjour"),
    (&["bonsoir"], "Bonsoir"),
    (&["salut"], "Salut"),
    (&["coucou"], "Salut"),
    (&["hola"], "Hola"),
    (&["buenas"], "Hola"),
    (&["ciao"], "Ciao"),
    (&["hallo"], "Hallo"),
    (&["hello"], "Hello"),
    (&["hi"], "Hello"),
    (&["hey"], "Hello"),
    (&["hiya"], "Hello"),
    (&["yo"], "Hello"),
    (&["greetings"], "Hello"),
    (&["howdy"], "Hello"),
];

/// Words that add nothing to what is asked: they are dropped before a request is compared with the phrases below.
const FILLER: &[&str] = &[
    "please",
    "pls",
    "plz",
    "kindly",
    "dear",
    "there",
    "everyone",
    "everybody",
    "again",
    "my",
    "friend",
    "bro",
    "buddy",
    "team",
    "so",
    "very",
];

const IDENTITY: &[&str] = &[
    "who are you",
    "who r you",
    "who is this",
    "who is that",
    "what are you",
    "who is cairn",
    "who's cairn",
    "whos cairn",
    "what is cairn",
    "what's cairn",
    "whats cairn",
    "tell me about cairn",
    "tell me about yourself",
    "introduce yourself",
    "your name",
    "what is your name",
    "what's your name",
    "whats your name",
    "what should i call you",
    "what do i call you",
    "are you a bot",
    "are you an ai",
    "are you ai",
    "are you a robot",
    "are you real",
    "who made you",
    "who built you",
    "qui es tu",
    "qui es-tu",
    "tu es qui",
];

const CAPABILITIES: &[&str] = &[
    "what can you do",
    "what can you do here",
    "what can you do for me",
    "what do you do",
    "what can you help with",
    "what can you help me with",
    "what can you help us with",
    "how can you help",
    "how can you help me",
    "how can you help us",
    "what are your capabilities",
    "what are you capable of",
    "what are you able to do",
    "what are you good at",
    "what are your features",
    "what are your skills",
    "what are your functions",
    "what can i ask",
    "what can i ask you",
    "what can i ask cairn",
    "what can i do here",
    "how do i use you",
    "how do i use this",
    "how does this work",
    "how do you work",
    "show me what you can do",
    "tell me what you can do",
    "list your capabilities",
    "what do you know",
    "help",
    "help me",
    "i need help",
    "need help",
    "capabilities",
    "features",
    "commands",
    "options",
    "menu",
    "que peux tu faire",
    "que peux-tu faire",
    "tu peux faire quoi",
    "aide",
];

const THANKS: &[&str] = &[
    "thanks",
    "thank you",
    "thank u",
    "thx",
    "ty",
    "tysm",
    "thanks a lot",
    "thanks a bunch",
    "thanks so much",
    "thank you so much",
    "thank you very much",
    "many thanks",
    "great thanks",
    "ok thanks",
    "okay thanks",
    "perfect thanks",
    "awesome thanks",
    "cheers",
    "merci",
    "merci beaucoup",
    "shukran",
    "shukran jazilan",
    "jazakallah khair",
];

const FAREWELLS: &[&str] = &[
    "bye",
    "bye bye",
    "goodbye",
    "good bye",
    "see you",
    "see you later",
    "see you soon",
    "see ya",
    "cya",
    "good night",
    "take care",
    "au revoir",
    "a bientot",
    "à bientôt",
    "ma salama",
    "bslama",
];

const HOW_ARE_YOU: &[&str] = &[
    "how are you",
    "how r you",
    "how are you doing",
    "how you doing",
    "how are you today",
    "how is it going",
    "how's it going",
    "hows it going",
    "how are things",
    "what's up",
    "whats up",
    "wassup",
    "sup",
    "ca va",
    "ça va",
    "comment ca va",
    "comment ça va",
    "kif halek",
    "kifak",
    "labas",
];

/// The words of a message, lowercased, with punctuation dropped and apostrophes kept inside a word.
fn words(message: &str) -> Vec<String> {
    message
        .to_lowercase()
        .replace('’', "'")
        .split(|c: char| !(c.is_alphanumeric() || c == '\''))
        .map(|word| word.trim_matches('\'').to_owned())
        .filter(|word| !word.is_empty())
        .collect()
}

fn listed(table: &[&str], tokens: &[String]) -> bool {
    let joined = tokens.join(" ");
    // "who is cairn" and "who is" mean the same thing: the name is one more way of addressing CAIRN.
    let without_name = tokens
        .iter()
        .filter(|token| token.as_str() != "cairn")
        .cloned()
        .collect::<Vec<_>>()
        .join(" ");
    table
        .iter()
        .any(|phrase| *phrase == joined || (!without_name.is_empty() && *phrase == without_name))
}

/// Recognises a message that is only chatter: a greeting, optionally followed by one of the other kinds. A message that says
/// more than a listed phrase is never chatter, so "who are you looking for in the logs" is left to the router.
pub(crate) fn recognise_chatter(message: &str) -> Option<Chatter> {
    let all = words(message);
    if all.is_empty() || all.len() > 10 {
        return None;
    }
    let mut greeting = None;
    let mut rest = all.as_slice();
    // "cairn" in front of the message addresses CAIRN; a greeting may follow it.
    if rest.first().is_some_and(|word| word == "cairn") && rest.len() > 1 {
        rest = &rest[1..];
    }
    for (phrase, word) in GREETINGS {
        if rest.len() >= phrase.len() && phrase.iter().zip(rest).all(|(expected, got)| expected == got) {
            greeting = Some(*word);
            rest = &rest[phrase.len()..];
            break;
        }
    }
    let remainder = rest
        .iter()
        .filter(|word| !FILLER.contains(&word.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    let remainder_without_name = remainder
        .iter()
        .filter(|word| word.as_str() != "cairn")
        .collect::<Vec<_>>();
    if remainder_without_name.is_empty() {
        return greeting.map(|word| Chatter {
            kind: ChatterKind::Greeting,
            greeting: Some(word),
        });
    }
    let kind = if listed(HOW_ARE_YOU, &remainder) {
        ChatterKind::HowAreYou
    } else if listed(IDENTITY, &remainder) {
        ChatterKind::Identity
    } else if listed(CAPABILITIES, &remainder) {
        ChatterKind::Capabilities
    } else if listed(THANKS, &remainder) {
        ChatterKind::Thanks
    } else if listed(FAREWELLS, &remainder) {
        ChatterKind::Farewell
    } else {
        return None;
    };
    Some(Chatter { kind, greeting })
}

/// One example of what to type for each job, so the list shows how to ask as well as what is possible.
fn example(capability: AgentCapability) -> &'static str {
    match capability {
        AgentCapability::FindFiles => "find the file about login",
        AgentCapability::FindSymbol => "where is parse_config defined",
        AgentCapability::SearchContent => "search for retry",
        AgentCapability::SummarizeFile => "summarize README.md",
        AgentCapability::AskQuestion => "how does authentication work?",
        AgentCapability::WorkspaceOverview => "give me a workspace overview",
        AgentCapability::SaveMemory => "save that as a memory",
        AgentCapability::ManageFiles => "move retry-policy.md to Runbooks",
        AgentCapability::SummarizeChat => "summarize this chat",
        AgentCapability::AnalyzeFiles => "analyze the Word documents",
        AgentCapability::RecallMemory => "what do we know about retries",
        AgentCapability::ManageMemory => "rename the memory card “Retry policy” to Backoff policy",
        AgentCapability::CreateNote => "create a note titled Pager rota: Alice is on call in March",
    }
}

/// The jobs CAIRN can do, as a Markdown list read from the capability list, each with something to type. Jobs that need an AI
/// provider say so when none is enabled.
fn jobs_markdown(context: &ChatterContext) -> String {
    let mut lines = agent_capabilities(context.ai_available)
        .into_iter()
        .map(|info| {
            let needs = if info.available {
                String::new()
            } else {
                " (needs an AI provider, and none is enabled here)".to_owned()
            };
            format!(
                "- **{}**: {}{needs} Try `{}`.",
                info.label,
                info.description,
                example(info.id)
            )
        })
        .collect::<Vec<_>>();
    if !context.ai_available {
        lines.push(String::new());
        lines.push("No AI provider is enabled for this workspace, so for now I can find files and symbols and search their content. An administrator can enable one in Settings.".to_owned());
    }
    lines.join("\n")
}

/// CAIRN's own answer to a chatty message. It says nothing about the workspace's files, so nothing in it needs a source.
pub(crate) fn chatter_reply(chatter: Chatter, context: &ChatterContext) -> AgentReply {
    let jobs = jobs_markdown(context);
    let ideas = context.ideas_markdown();
    let workspace = context.workspace_clause();
    let opener = chatter.greeting.map(|word| format!("{word}! ")).unwrap_or_default();
    let markdown = match chatter.kind {
        ChatterKind::Greeting => format!(
            "{opener}I'm CAIRN, the assistant for {workspace}. I can help you find and understand what is in its indexed files:\n\n{jobs}{ideas}\n\nWhat would you like to do?"
        ),
        ChatterKind::Identity => format!(
            "{opener}I'm CAIRN, the assistant built into RepoMemo. I work inside {workspace} and I only know what its indexed files say, so what I tell you comes with the files it came from. Here is what I can do:\n\n{jobs}"
        ),
        ChatterKind::Capabilities => format!(
            "{opener}Here is what I can do in {workspace}:\n\n{jobs}{ideas}\n\nType what you need, or pick one of these."
        ),
        ChatterKind::HowAreYou => format!("{opener}I'm doing well, thank you. What would you like to look up in this workspace?"),
        ChatterKind::Thanks => {
            "You're welcome! Ask me about this workspace's files whenever you like.".to_owned()
        }
        ChatterKind::Farewell => "Goodbye! I'll be here whenever you need to find something.".to_owned(),
    };
    let mut answer = reply(markdown);
    answer.routing = AgentRouting::Rules;
    answer
}

/// What CAIRN's AI is told when a message asks for none of its jobs (V0.9.2). It has no passages to read for such a message, so
/// it speaks for itself and states nothing about the files.
pub(crate) const CHAT_RULES: &str = "The user's message asks for none of your jobs, and you have been given no passages from the workspace for it. Reply as CAIRN in a few short sentences. Do not state facts about the workspace's files, and never invent file names, code or sources. You cannot write or edit files or code, run anything, or take actions; if the user asks for that, say so plainly and point to the closest job you can do. For small talk, answer naturally and offer to help with the workspace.";

/// The system prompt of CAIRN's own replies: its persona, then [`CHAT_RULES`].
pub(crate) fn chat_system() -> String {
    format!("{} {CHAT_RULES}", super::cairn_persona())
}

/// What CAIRN says to a message that asks for none of its jobs when no AI can answer it: it says so in its own voice and shows
/// what it can do instead of a bare refusal.
pub(crate) fn unmatched_reply(context: &ChatterContext) -> AgentReply {
    let mut answer = reply(format!(
        "I'm not sure how to help with that one. Here is what I can do in {}:\n\n{}\n\nTry rephrasing your request as one of these.",
        context.workspace_clause(),
        jobs_markdown(context)
    ));
    answer.routing = AgentRouting::Unmatched;
    answer
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unmatched_reply_speaks_as_cairn_and_shows_what_it_can_do() {
        let answer = unmatched_reply(&ChatterContext {
            ai_available: false,
            workspace: None,
            ..Default::default()
        });
        assert!(answer.reply_markdown.starts_with("I'm not sure how to help"));
        assert!(answer.reply_markdown.contains("Find files") && answer.reply_markdown.contains("No AI provider"));
        assert_eq!(answer.routing, AgentRouting::Unmatched);
        assert!(!answer.reply_markdown.contains("can't help with that yet"));
    }

    #[test]
    fn the_chat_prompt_is_cairn_and_forbids_inventing_the_workspace() {
        let system = chat_system();
        assert!(system.starts_with("You are CAIRN"));
        assert!(system.contains("never invent file names"));
        assert!(system.contains("cannot write or edit files"));
    }

    fn kind(message: &str) -> Option<ChatterKind> {
        recognise_chatter(message).map(|chatter| chatter.kind)
    }

    #[test]
    fn greetings_in_several_languages_are_greetings_and_are_greeted_back() {
        for (message, word) in [
            ("Salam", "Salam"),
            ("salam alaykum!", "Salam"),
            ("Hello", "Hello"),
            ("hey there", "Hello"),
            ("Hi CAIRN", "Hello"),
            ("cairn hello", "Hello"),
            ("Bonjour", "Bonjour"),
            ("hola", "Hola"),
            ("good morning everyone", "Good morning"),
        ] {
            let chatter = recognise_chatter(message).unwrap_or_else(|| panic!("{message} is a greeting"));
            assert_eq!(chatter.kind, ChatterKind::Greeting, "{message}");
            assert_eq!(chatter.greeting, Some(word), "{message}");
        }
    }

    #[test]
    fn who_are_you_and_what_can_you_do_are_chatter() {
        assert_eq!(kind("who are you?"), Some(ChatterKind::Identity));
        assert_eq!(kind("Who is CAIRN"), Some(ChatterKind::Identity));
        assert_eq!(kind("what is your name?"), Some(ChatterKind::Identity));
        assert_eq!(kind("what can you do"), Some(ChatterKind::Capabilities));
        assert_eq!(kind("What can you do, please?"), Some(ChatterKind::Capabilities));
        assert_eq!(kind("help"), Some(ChatterKind::Capabilities));
        assert_eq!(kind("hey cairn, what can you do?"), Some(ChatterKind::Capabilities));
        assert_eq!(kind("how are you"), Some(ChatterKind::HowAreYou));
        assert_eq!(kind("thanks!"), Some(ChatterKind::Thanks));
        assert_eq!(kind("thank you so much"), Some(ChatterKind::Thanks));
        assert_eq!(kind("bye"), Some(ChatterKind::Farewell));
        assert_eq!(
            recognise_chatter("salam who are you").and_then(|chatter| chatter.greeting),
            Some("Salam")
        );
    }

    #[test]
    fn a_message_that_says_more_than_a_phrase_is_left_to_the_router() {
        for message in [
            "who are you looking for in the logs",
            "what can you do about the failing build",
            "help me find the login file",
            "hello world function",
            "how does authentication work?",
            "find the file about login",
            "thanks for the summary, now summarize the readme",
            "",
            "   ",
        ] {
            assert_eq!(recognise_chatter(message), None, "{message}");
        }
    }

    #[test]
    fn the_list_of_jobs_comes_from_the_capability_list_and_says_what_needs_ai() {
        let with_ai = chatter_reply(
            recognise_chatter("what can you do").unwrap(),
            &ChatterContext {
                ai_available: true,
                workspace: None,
                ..Default::default()
            },
        );
        for info in agent_capabilities(true) {
            assert!(with_ai.reply_markdown.contains(&info.label), "{}", info.label);
        }
        assert!(!with_ai.reply_markdown.contains("none is enabled"));
        assert_eq!(with_ai.routing, AgentRouting::Rules);
        assert!(!with_ai.generated && with_ai.capability.is_none());

        let without_ai = chatter_reply(
            recognise_chatter("what can you do").unwrap(),
            &ChatterContext {
                ai_available: false,
                workspace: None,
                ..Default::default()
            },
        );
        assert!(without_ai.reply_markdown.contains("needs an AI provider"));
        assert!(without_ai.reply_markdown.contains("No AI provider is enabled"));
    }

    #[test]
    fn a_greeting_names_the_workspace_and_what_is_indexed_in_it() {
        let in_workspace = |name: &str, files: i64| ChatterContext {
            ai_available: true,
            workspace: Some(WorkspaceBrief {
                name: name.to_owned(),
                files,
            }),
            ..Default::default()
        };
        let greet = |files: i64| {
            chatter_reply(
                recognise_chatter("Salam").unwrap(),
                &in_workspace("Engineering / Bendly", files),
            )
            .reply_markdown
        };
        assert!(
            greet(42).starts_with("Salam! I'm CAIRN, the assistant for **Engineering / Bendly** (42 indexed files).")
        );
        assert!(greet(1).contains("(1 indexed file)"));
        assert!(greet(0).contains("(nothing is indexed yet)"));
        let identity = chatter_reply(recognise_chatter("who are you").unwrap(), &in_workspace("Docs", 3));
        assert!(identity
            .reply_markdown
            .contains("I work inside **Docs** (3 indexed files)"));
    }

    #[test]
    fn a_templates_questions_follow_the_jobs_in_a_greeting_and_in_what_can_you_do() {
        let context = ChatterContext {
            ai_available: true,
            suggestions: vec![
                "What is the runbook for…".to_owned(),
                "  ".to_owned(),
                "Who is on call?".to_owned(),
                "Where is the pager policy?".to_owned(),
                "A fourth one".to_owned(),
            ],
            ..Default::default()
        };
        for message in ["hello", "what can you do"] {
            let shown = chatter_reply(recognise_chatter(message).unwrap(), &context).reply_markdown;
            assert!(
                shown.contains(
                    "set up for questions like:\n- What is the runbook for…\n- Who is on call?\n- Where is the pager policy?"
                ),
                "{shown}"
            );
            assert!(!shown.contains("A fourth one"), "only three are shown");
        }
        let without = chatter_reply(recognise_chatter("hello").unwrap(), &ChatterContext::default()).reply_markdown;
        assert!(!without.contains("set up for questions"));
    }

    #[test]
    fn a_greeting_is_answered_in_kind_and_introduces_cairn() {
        let answer = chatter_reply(
            recognise_chatter("Salam").unwrap(),
            &ChatterContext {
                ai_available: true,
                workspace: None,
                ..Default::default()
            },
        );
        assert!(answer.reply_markdown.starts_with("Salam! I'm CAIRN"));
        let thanks = chatter_reply(recognise_chatter("thanks").unwrap(), &ChatterContext::default());
        assert!(thanks.reply_markdown.starts_with("You're welcome"));
    }
}

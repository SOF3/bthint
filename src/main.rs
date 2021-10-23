use std::convert::TryFrom;
use std::future::Future;

use serde::Deserialize;
use serenity::client::Context;
use serenity::model::*;

mod php;

type Result<T = (), E = anyhow::Error> = std::result::Result<T, E>;

#[tokio::main]
async fn main() -> Result<()> {
    pretty_env_logger::init();

    let config = load_config()?;
    let token = config.discord.token.to_owned();
    let handler = Handler::try_from(config)?;
    let mut client = serenity::Client::builder(token)
        .event_handler(handler)
        .await
        .expect("Error connecting to discord");
    client.start().await.map_err(Into::into)
}

fn load_config() -> Result<Config> {
    let mut config = config::Config::new();
    config.merge(config::File::with_name("config"))?;
    config.try_into().map_err(Into::into)
}

#[derive(Deserialize)]
struct Config {
    discord: DiscordConfig,
}

#[derive(Deserialize)]
struct DiscordConfig {
    client_id: u64,
    target_guild: u64,
    token: String,
}

struct Handler {
    client_id: u64,
    target_guild: u64,
    invite_link: String,
}

impl TryFrom<Config> for Handler {
    type Error = anyhow::Error;

    fn try_from(config: Config) -> Result<Self> {
        let Config {
            discord:
                DiscordConfig {
                    client_id,
                    target_guild,
                    ..
                },
        } = config;

        let invite_link = format!(
            "https://discord.com/oauth2/authorize?client_id={}&scope=bot",
            client_id
        );
        log::info!("Invite link: {}", &invite_link);
        Ok(Self {
            client_id,
            target_guild,
            invite_link,
        })
    }
}

#[async_trait::async_trait]
impl serenity::client::EventHandler for Handler {
    async fn message(&self, ctx: Context, message: channel::Message) {
        if let Some(guild) = message.guild_id {
            if guild != self.target_guild {
                return;
            }
        }
        if message.author.id == self.client_id {
            return;
        }
        if &message.content == "bthint invite" {
            let reply = format!("Invite link: {}", &self.invite_link);
            if let Err(err) = message.reply(&ctx, reply).await {
                log::error!("Error replying to discord: {}", err);
            }
        } else if !message.content.contains("```") {
            if let Some((lang, code)) = detect_lang(&message.content).await {
                let reply = format!(
                    r#"Hint: use three backticks \`\`\` to wrap your code.
So this:
\`\`\`{}
{}
\`\`\`
Turns into this:
```{0}
{1}
```"#,
                    lang, code
                );
                if let Err(err) = message.reply(&ctx, reply).await {
                    log::error!("Error replying to discord: {}", err);
                }
            }
        }
    }
}

async fn detect_lang(message: &str) -> Option<(&'static str, String)> {
    let php = php::verify_php(message);
    // TODO other languages
    if let Some(code) = php.await {
        return Some(("php", code));
    }
    None
}

#[allow(dead_code)]
async fn trying<R, F>(f: impl FnOnce() -> F) -> Result<R>
where
    F: Future<Output = Result<R>>,
{
    f().await
}

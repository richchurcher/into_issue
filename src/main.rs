/// Retrieve a PR, copy its description and tags to an issue which links back to the PR,
/// effectively archiving it and reducing the PR backlog without entirely abandoning it.
use anyhow::{Context, Result};
use clap::Parser;
use reqwest::header::{AUTHORIZATION, USER_AGENT};
use serde::{Deserialize, Serialize};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    org: String,

    #[arg(short, long)]
    repo: String,

    #[arg(short, long)]
    pr_number: u32,

    #[arg(short, long)]
    token: String,
}

#[derive(Deserialize)]
struct Issue {
    html_url: String,
}

#[derive(Deserialize)]
struct Label {
    name: String,
}

#[derive(Serialize)]
struct NewIssue {
    title: String,
    body: String,
    labels: Vec<String>,
}

#[derive(Deserialize)]
struct PullRequest {
    body: Option<String>,
    labels: Vec<Label>,
    title: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let pr_url = format!(
        "https://api.github.com/repos/{org}/{repo}/pulls/{pr_number}",
        org = args.org,
        repo = args.repo,
        pr_number = args.pr_number
    );

    let client = reqwest::Client::new();
    let pr_response = client
        .get(&pr_url)
        .header(USER_AGENT, "into_issue")
        .header(AUTHORIZATION, format!("token {}", args.token))
        .send()
        .await
        .context("Failed to send request to fetch pull request details")?;

    if pr_response.status().is_success() {
        let pr: PullRequest = pr_response
            .json()
            .await
            .context("Failed to deserialize pull request response")?;

        let Some(description) = pr.body else {
            anyhow::bail!("PR did not appear to have a description, exiting.");
        };
        // TODO: extract this template somewhere
        let issue_body = format!(
                "> [!NOTE]\n
                > _This issue is a reference to an older PR (#{pr_number}) which has now been closed. If it
                could still be valuable to the project, it can be adopted and worked on in a new
                PR. If you feel that the target PR is no longer viable, and the feature is unwanted
                or already exists, this issue can be closed._\n
                > \n
                > _The original PR description is included below._\n
                \n
                {description}\n",
                pr_number = args.pr_number,
            );

        let labels = pr.labels.iter().map(|label| label.name.clone()).collect();
        let issue_url = format!(
            "https://api.github.com/repos/{org}/{repo}/issues",
            org = args.org,
            repo = args.repo
        );
        let new_issue = NewIssue {
            title: format!(
                "{} (tracking issue for closed PR #{})",
                pr.title, args.pr_number
            ),
            body: issue_body,
            labels,
        };

        let issue_response = client
            .post(&issue_url)
            .header(USER_AGENT, "into_issue")
            .header(AUTHORIZATION, format!("token {}", args.token))
            .json(&new_issue)
            .send()
            .await
            .context("Failed to send request to create issue")?;

        if issue_response.status().is_success() {
            let issue: Issue = issue_response
                .json()
                .await
                .context("Failed to deserialize issue creation response.")?;

            println!("Issue created: {}", issue.html_url);
        } else {
            anyhow::bail!("Failed to create the issue: {}", issue_response.status());
        }
    } else {
        anyhow::bail!("Failed to fetch pull request: {}", pr_response.status());
    }

    Ok(())
}

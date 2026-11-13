use std::sync::Arc;

use dejafix_core::{McpState, OsIds, Outcome, ToolCall, ToolReply, WriteCall};
use rmcp::ErrorData;
use rmcp::handler::server::ServerHandler;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, Implementation,
    ListToolsResult, PaginatedRequestParams, ServerCapabilities, ServerConfig,
};
use rmcp::service::{RequestContext, RoleServer};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use super::schema::instructions;
use super::types::{
    ArgumentError, AttemptArgs, ContextArgs, DecisionArgs, DejafixServer, FileArgs, FilesArgs,
    FixArgs, GotchaArgs, IssueArgs, ProjectArgs, SearchArgs, WriteArgs,
};

const SERVER_NAME: &str = "dejafix";

fn parse<T: DeserializeOwned>(arguments: &Value) -> Result<T, ArgumentError> {
    Ok(T::deserialize(arguments)?)
}

fn within(
    value: i64,
    range: std::ops::RangeInclusive<i64>,
    field: &'static str,
    expected: &'static str,
) -> Result<i64, ArgumentError> {
    if range.contains(&value) {
        Ok(value)
    } else {
        Err(ArgumentError::Range { field, expected })
    }
}

fn write(arguments: &Value) -> Result<(String, Option<String>), ArgumentError> {
    let WriteArgs { summary, location } = parse(arguments)?;
    Ok((summary, location))
}

fn tool_call(name: &str, arguments: &Value) -> Result<ToolCall, ArgumentError> {
    Ok(match name {
        "start_session" => ToolCall::StartSession,
        "get_instructions" => ToolCall::GetInstructions,
        "get_summary" => ToolCall::GetSummary,
        "get_project_map" => ToolCall::GetProjectMap,
        "get_plan" => ToolCall::GetPlan,
        "get_score" => ToolCall::GetScore,
        "list_projects" => ToolCall::ListProjects,
        "current_project" => ToolCall::CurrentProject,
        "precheck_file" => ToolCall::PrecheckFile(parse::<FileArgs>(arguments)?.file_path),
        "precheck_files" => ToolCall::PrecheckFiles(parse::<FilesArgs>(arguments)?.file_paths),
        "get_issue" => ToolCall::GetIssue(parse::<IssueArgs>(arguments)?.issue_id),
        "search_events" => {
            let SearchArgs { query, limit, full } = parse(arguments)?;
            let limit = within(
                limit,
                1..=100,
                "limit",
                "greater than or equal to 1 and less than or equal to 100",
            )?;
            ToolCall::SearchEvents {
                query,
                limit: usize::try_from(limit).unwrap_or(1),
                full,
            }
        }
        "get_context" => {
            let ContextArgs { tokens, focus } = parse(arguments)?;
            let tokens = within(
                tokens,
                100..=20_000,
                "tokens",
                "greater than or equal to 100 and less than or equal to 20000",
            )?;
            ToolCall::GetContext {
                tokens: u32::try_from(tokens).unwrap_or(100),
                focus,
            }
        }
        "get_global_gotchas" => ToolCall::GetGlobalGotchas(parse::<GotchaArgs>(arguments)?.library),
        "log_issue" => {
            let (summary, location) = write(arguments)?;
            ToolCall::Write(WriteCall::LogIssue { summary, location })
        }
        "record_attempt" => {
            let (summary, location) = write(arguments)?;
            let AttemptArgs { outcome, issue_id } = parse(arguments)?;
            let outcome = Outcome::parse(&outcome).ok_or(ArgumentError::Range {
                field: "outcome",
                expected: "one of 'worked', 'failed' or 'partial'",
            })?;
            ToolCall::Write(WriteCall::RecordAttempt {
                summary,
                outcome,
                location,
                issue_id,
            })
        }
        "record_fix" => {
            let (summary, location) = write(arguments)?;
            ToolCall::Write(WriteCall::RecordFix {
                summary,
                location,
                issue_id: parse::<FixArgs>(arguments)?.issue_id,
            })
        }
        "add_decision" => {
            let (summary, location) = write(arguments)?;
            ToolCall::Write(WriteCall::AddDecision {
                summary,
                location,
                supersedes: parse::<DecisionArgs>(arguments)?.supersedes,
            })
        }
        "add_note" => {
            let (summary, location) = write(arguments)?;
            ToolCall::Write(WriteCall::AddNote { summary, location })
        }
        _ => return Err(ArgumentError::Unknown),
    })
}

fn failure(text: String) -> CallToolResult {
    let mut result = CallToolResult::success(vec![ContentBlock::text(text)]);
    result.is_error = Some(true);
    result
}

fn reply(reply: ToolReply) -> CallToolResult {
    let structured = json!({ "result": reply.text });
    let mut result = CallToolResult::success(vec![ContentBlock::text(reply.text)]);
    result.structured_content = Some(structured);
    result.is_error = Some(reply.error);
    result
}

impl DejafixServer {
    pub fn new(state: McpState) -> Result<Self, serde_json::Error> {
        Ok(Self {
            state: Arc::new(state),
            tools: super::schema::tools()?,
        })
    }

    pub async fn run(&self, name: &str, arguments: Value) -> CallToolResult {
        let call = match tool_call(name, &arguments) {
            Ok(call) => call,
            Err(ArgumentError::Unknown) => return failure(format!("Unknown tool: {name}")),
            Err(error) => return failure(format!("Error executing tool {name}: {error}")),
        };
        let project = if call == ToolCall::ListProjects {
            None
        } else {
            match parse::<ProjectArgs>(&arguments) {
                Ok(arguments) => arguments.project,
                Err(error) => return failure(format!("Error executing tool {name}: {error}")),
            }
        };
        let state = Arc::clone(&self.state);
        let joined = tokio::task::spawn_blocking(move || {
            state.call(call, project.as_deref(), jiff::Timestamp::now(), &mut OsIds)
        })
        .await;
        match joined {
            Ok(result) => reply(result),
            Err(error) => failure(format!("dejafix tool error: JoinError: {error}")),
        }
    }
}

impl ServerHandler for DejafixServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(SERVER_NAME, env!("CARGO_PKG_VERSION")))
            .with_instructions(instructions(self.state.global_mode()))
    }

    #[expect(
        clippy::unused_async_trait_impl,
        reason = "the handler trait declares an async method"
    )]
    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult::with_all_items(self.tools.to_vec()))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let arguments = Value::Object(request.arguments.unwrap_or_default());
        Ok(self.run(&request.name, arguments).await.into())
    }
}

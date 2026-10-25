use super::turn::Turn;
use crate::domain::event::{Event, ToolCall, ToolResult};

/// Port of openfox `executeTools`, sequential for now.
pub(super) fn execute_tools(
    turn: &mut Turn,
    message_id: &str,
    calls: &[ToolCall],
) -> Result<(), String> {
    for call in calls {
        let result = match &call.parse_error {
            Some(pe) => ToolResult::failure(format!(
                "Failed to parse tool call arguments: {pe}. Please ensure your JSON function call arguments are valid."
            )),
            None => {
                let (tools, stop) = (turn.tools, turn.stop);
                let mut log_error = None;
                let r = tools.execute(&call.name, &call.arguments, stop, &mut |stream, chunk| {
                    if log_error.is_none() {
                        log_error = turn
                            .append(Event::ToolOutput {
                                message_id: message_id.into(),
                                tool_call_id: call.id.clone(),
                                stream,
                                content: chunk.into(),
                            })
                            .err();
                    }
                });
                if let Some(e) = log_error {
                    return Err(e);
                }
                r
            }
        };
        turn.append(Event::ToolResult {
            message_id: message_id.into(),
            tool_call_id: call.id.clone(),
            result,
        })?;
    }
    Ok(())
}

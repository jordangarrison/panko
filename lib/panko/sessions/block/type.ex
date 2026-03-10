defmodule Panko.Sessions.Block.Type do
  use Ash.Type.Enum,
    values: [
      :user_prompt,
      :assistant_response,
      :tool_call,
      :thinking,
      :file_edit,
      :sub_agent_spawn
    ]
end

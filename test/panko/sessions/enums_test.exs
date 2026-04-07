defmodule Panko.Sessions.EnumsTest do
  use ExUnit.Case, async: true

  alias Panko.Sessions.SourceType
  alias Panko.Sessions.Block.Type, as: BlockType
  alias Panko.Sessions.SubAgentStatus

  describe "SourceType" do
    test "has expected values" do
      assert :claude_code in SourceType.values()
      assert :codex in SourceType.values()
    end

    test "casts valid string" do
      assert {:ok, :claude_code} = Ash.Type.cast_input(SourceType, "claude_code")
    end

    test "rejects invalid value" do
      assert {:error, _} = Ash.Type.cast_input(SourceType, "invalid")
    end
  end

  describe "BlockType" do
    test "has all block types" do
      values = BlockType.values()
      assert :user_prompt in values
      assert :assistant_response in values
      assert :tool_call in values
      assert :thinking in values
      assert :file_edit in values
      assert :sub_agent_spawn in values
    end
  end

  describe "SubAgentStatus" do
    test "has expected values" do
      values = SubAgentStatus.values()
      assert :running in values
      assert :completed in values
      assert :failed in values
    end
  end
end

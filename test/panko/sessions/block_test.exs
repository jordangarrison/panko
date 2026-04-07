defmodule Panko.Sessions.BlockTest do
  use Panko.DataCase, async: true

  alias Panko.Sessions.{Session, Block}

  setup do
    {:ok, session} =
      Session
      |> Ash.Changeset.for_create(:create, %{
        external_id: "block-test-session",
        source_type: :claude_code,
        started_at: ~U[2026-03-09 12:00:00Z]
      })
      |> Ash.create()

    %{session: session}
  end

  describe "create" do
    test "creates a user_prompt block", %{session: session} do
      assert {:ok, block} =
               Block
               |> Ash.Changeset.for_create(:create, %{
                 session_id: session.id,
                 position: 0,
                 block_type: :user_prompt,
                 content: "Hello, Claude!",
                 timestamp: ~U[2026-03-09 12:00:01Z]
               })
               |> Ash.create()

      assert block.block_type == :user_prompt
      assert block.content == "Hello, Claude!"
      assert block.position == 0
    end

    test "creates a tool_call block with metadata", %{session: session} do
      metadata = %{
        "name" => "Bash",
        "input" => %{"command" => "ls"},
        "output" => %{"text" => "file.txt"}
      }

      assert {:ok, block} =
               Block
               |> Ash.Changeset.for_create(:create, %{
                 session_id: session.id,
                 position: 1,
                 block_type: :tool_call,
                 content: nil,
                 metadata: metadata,
                 timestamp: ~U[2026-03-09 12:00:02Z]
               })
               |> Ash.create()

      assert block.metadata["name"] == "Bash"
    end

    test "enforces unique session_id + position", %{session: session} do
      attrs = %{
        session_id: session.id,
        position: 0,
        block_type: :user_prompt,
        content: "First",
        timestamp: ~U[2026-03-09 12:00:01Z]
      }

      assert {:ok, _} = Block |> Ash.Changeset.for_create(:create, attrs) |> Ash.create()
      assert {:error, _} = Block |> Ash.Changeset.for_create(:create, attrs) |> Ash.create()
    end
  end

  describe "session aggregates" do
    test "counts blocks by type", %{session: session} do
      for {type, pos} <- [
            {:user_prompt, 0},
            {:assistant_response, 1},
            {:tool_call, 2},
            {:tool_call, 3}
          ] do
        Block
        |> Ash.Changeset.for_create(:create, %{
          session_id: session.id,
          position: pos,
          block_type: type,
          timestamp: ~U[2026-03-09 12:00:00Z]
        })
        |> Ash.create!()
      end

      session =
        Session
        |> Ash.get!(session.id, load: [:block_count, :tool_call_count, :message_count])

      assert session.block_count == 4
      assert session.tool_call_count == 2
      assert session.message_count == 2
    end
  end
end

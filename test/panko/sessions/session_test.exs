defmodule Panko.Sessions.SessionTest do
  use Panko.DataCase, async: true

  alias Panko.Sessions.Session

  describe "create" do
    test "creates a session with valid attributes" do
      assert {:ok, session} =
               Session
               |> Ash.Changeset.for_create(:create, %{
                 external_id: "test-session-123",
                 source_type: :claude_code,
                 source_path: "/tmp/test.jsonl",
                 project: "my-project",
                 title: "Test session",
                 started_at: ~U[2026-03-09 12:00:00Z]
               })
               |> Ash.create()

      assert session.external_id == "test-session-123"
      assert session.source_type == :claude_code
      assert session.project == "my-project"
    end

    test "requires external_id and source_type" do
      assert {:error, _} =
               Session
               |> Ash.Changeset.for_create(:create, %{
                 started_at: ~U[2026-03-09 12:00:00Z]
               })
               |> Ash.create()
    end
  end
end

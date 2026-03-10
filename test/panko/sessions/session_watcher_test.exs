defmodule Panko.Sessions.SessionWatcherTest do
  use Panko.DataCase, async: false

  alias Panko.Sessions.SessionWatcher

  @fixtures_dir Path.join([__DIR__, "../../fixtures"])

  @tag :tmp_dir
  test "initial scan imports existing JSONL files", %{tmp_dir: tmp_dir} do
    # Copy fixture into tmp dir
    fixture = File.read!(Path.join(@fixtures_dir, "simple_session.jsonl"))
    jsonl_path = Path.join(tmp_dir, "session.jsonl")
    File.write!(jsonl_path, fixture)

    # Start watcher pointing at tmp dir (unique name to avoid conflicts)
    {:ok, pid} =
      SessionWatcher.start_link(
        watch_paths: [tmp_dir],
        name: :"watcher_#{System.unique_integer([:positive])}"
      )

    # Give it time to scan and import
    Process.sleep(1_500)

    # Verify session was imported
    sessions = Ash.read!(Panko.Sessions.Session)
    assert length(sessions) >= 1
    assert Enum.any?(sessions, &(&1.external_id == "test-abc-123"))

    GenServer.stop(pid)
  end

  @tag :tmp_dir
  test "watches for new files and imports them", %{tmp_dir: tmp_dir} do
    # Start watcher on empty dir
    {:ok, pid} =
      SessionWatcher.start_link(
        watch_paths: [tmp_dir],
        name: :"watcher_#{System.unique_integer([:positive])}"
      )

    # Wait for initial scan to finish
    Process.sleep(500)

    # Verify no sessions exist yet
    assert Ash.read!(Panko.Sessions.Session) == []

    # Now drop a file in
    fixture = File.read!(Path.join(@fixtures_dir, "simple_session.jsonl"))
    jsonl_path = Path.join(tmp_dir, "session.jsonl")
    File.write!(jsonl_path, fixture)

    # Wait for debounce + import (debounce is 2s)
    Process.sleep(4_000)

    # Verify session was imported
    sessions = Ash.read!(Panko.Sessions.Session)
    assert length(sessions) >= 1
    assert Enum.any?(sessions, &(&1.external_id == "test-abc-123"))

    GenServer.stop(pid)
  end

  test "start_link starts the process" do
    # Use a non-existent directory so no actual watching occurs
    {:ok, pid} =
      SessionWatcher.start_link(
        watch_paths: ["/tmp/panko_test_nonexistent_#{System.unique_integer([:positive])}"],
        name: :"watcher_#{System.unique_integer([:positive])}"
      )

    assert Process.alive?(pid)
    GenServer.stop(pid)
  end
end

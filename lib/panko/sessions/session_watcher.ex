defmodule Panko.Sessions.SessionWatcher do
  @moduledoc """
  Watches configured directories for new/modified JSONL session files
  and triggers import into the database.
  """
  use GenServer

  require Logger

  @debounce_ms 2_000

  def start_link(opts) do
    GenServer.start_link(__MODULE__, opts, name: opts[:name] || __MODULE__)
  end

  @impl true
  def init(opts) do
    watch_paths =
      opts[:watch_paths] ||
        Application.get_env(:panko, :session_watch_paths, "~/.claude/projects")
        |> List.wrap()
        |> Enum.map(&Path.expand/1)

    # Start file watchers for each path
    watchers =
      for path <- watch_paths, File.dir?(path) do
        {:ok, pid} = FileSystem.start_link(dirs: [path])
        FileSystem.subscribe(pid)
        pid
      end

    # Initial scan
    send(self(), :initial_scan)

    {:ok, %{watchers: watchers, watch_paths: watch_paths, pending: %{}}}
  end

  @impl true
  def handle_info(:initial_scan, state) do
    files = Enum.flat_map(state.watch_paths, &find_jsonl_files/1)

    Logger.info(
      "SessionWatcher: importing #{length(files)} files from #{length(state.watch_paths)} paths"
    )

    # Run initial scan in a task so the GenServer can still handle messages
    Task.start(fn ->
      files
      |> Task.async_stream(
        &do_import/1,
        max_concurrency: 4,
        timeout: :infinity,
        ordered: false
      )
      |> Stream.run()

      Logger.info("SessionWatcher: initial scan complete")
    end)

    {:noreply, state}
  end

  @impl true
  def handle_info({:file_event, _pid, {path, _events}}, state) do
    if String.ends_with?(path, ".jsonl") do
      # Debounce: schedule import after delay, reset if same file changes again
      timer = Process.send_after(self(), {:import, path}, @debounce_ms)

      state =
        case Map.get(state.pending, path) do
          nil ->
            state

          old_timer ->
            Process.cancel_timer(old_timer)
            state
        end

      {:noreply, put_in(state.pending[path], timer)}
    else
      {:noreply, state}
    end
  end

  @impl true
  def handle_info({:import, path}, state) do
    import_file(path)
    {:noreply, %{state | pending: Map.delete(state.pending, path)}}
  end

  @impl true
  def handle_info({:file_event, _pid, :stop}, state) do
    {:noreply, state}
  end

  defp find_jsonl_files(dir) do
    Path.wildcard(Path.join([dir, "**", "*.jsonl"]))
  end

  defp import_file(path) do
    Task.start(fn -> do_import(path) end)
  end

  defp do_import(path) do
    case Panko.Sessions.import_from_file(path) do
      {:ok, session} ->
        Logger.debug("Imported session #{session.external_id} from #{path}")

      {:error, reason} ->
        Logger.warning("Failed to import #{path}: #{inspect(reason)}")
    end
  end
end

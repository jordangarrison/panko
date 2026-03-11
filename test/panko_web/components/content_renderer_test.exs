defmodule PankoWeb.Components.ContentRendererTest do
  use ExUnit.Case, async: true

  alias PankoWeb.Components.ContentRenderer

  describe "parse_content/1" do
    test "plain text passes through as markdown" do
      segments = ContentRenderer.parse_content("Hello **world**")
      assert [{:markdown, "Hello **world**"}] = segments
    end

    test "extracts command-name tags" do
      input = "some text<command-name>foo</command-name>more text"
      segments = ContentRenderer.parse_content(input)

      assert [
               {:markdown, "some text"},
               {:command_name, "foo"},
               {:markdown, "more text"}
             ] = segments
    end

    test "extracts command blocks" do
      input = "before<command>do something</command>after"
      segments = ContentRenderer.parse_content(input)

      assert [
               {:markdown, "before"},
               {:command, "do something"},
               {:markdown, "after"}
             ] = segments
    end

    test "extracts command-message blocks" do
      input = "text<command-message>msg here</command-message>end"
      segments = ContentRenderer.parse_content(input)

      assert [
               {:markdown, "text"},
               {:command_message, "msg here"},
               {:markdown, "end"}
             ] = segments
    end

    test "extracts local-command-stdout blocks" do
      input = "before<local-command-stdout>output</local-command-stdout>after"
      segments = ContentRenderer.parse_content(input)

      assert [
               {:markdown, "before"},
               {:command_stdout, "output"},
               {:markdown, "after"}
             ] = segments
    end

    test "extracts local-command-caveat blocks" do
      input = "text<local-command-caveat>caveat text</local-command-caveat>rest"
      segments = ContentRenderer.parse_content(input)

      assert [
               {:markdown, "text"},
               {:command_caveat, "caveat text"},
               {:markdown, "rest"}
             ] = segments
    end

    test "extracts command-args blocks" do
      input = "text<command-args>--flag val</command-args>rest"
      segments = ContentRenderer.parse_content(input)

      assert [
               {:markdown, "text"},
               {:command_args, "--flag val"},
               {:markdown, "rest"}
             ] = segments
    end

    test "handles multiple tags in one string" do
      input = "start<command-name>ls</command-name> ran <command>ls -la</command>end"
      segments = ContentRenderer.parse_content(input)

      assert [
               {:markdown, "start"},
               {:command_name, "ls"},
               {:markdown, " ran "},
               {:command, "ls -la"},
               {:markdown, "end"}
             ] = segments
    end

    test "strips system-reminder tags entirely" do
      input = "visible<system-reminder>secret stuff</system-reminder>also visible"
      segments = ContentRenderer.parse_content(input)

      assert [
               {:markdown, "visible"},
               {:markdown, "also visible"}
             ] = segments
    end

    test "handles nil content" do
      assert [] = ContentRenderer.parse_content(nil)
    end

    test "handles empty content" do
      assert [] = ContentRenderer.parse_content("")
    end

    test "filters out empty markdown segments" do
      input = "<command-name>foo</command-name>"
      segments = ContentRenderer.parse_content(input)
      assert [{:command_name, "foo"}] = segments
    end
  end

  describe "render_markdown/1" do
    test "converts markdown to safe HTML" do
      result = ContentRenderer.render_markdown("Hello **world**")
      html = Phoenix.HTML.safe_to_string(result)
      assert html =~ "<strong>world</strong>"
    end

    test "handles code blocks" do
      result = ContentRenderer.render_markdown("```elixir\nIO.puts(\"hi\")\n```")
      html = Phoenix.HTML.safe_to_string(result)
      assert html =~ "<code"
    end

    test "sanitizes script tags from markdown" do
      result = ContentRenderer.render_markdown("<script>alert('xss')</script>")
      html = Phoenix.HTML.safe_to_string(result)
      refute html =~ "<script"
      refute html =~ "</script>"
    end

    test "sanitizes onerror attributes from markdown" do
      result = ContentRenderer.render_markdown(~S[<img src=x onerror="alert('xss')">])
      html = Phoenix.HTML.safe_to_string(result)
      refute html =~ "onerror"
    end
  end
end

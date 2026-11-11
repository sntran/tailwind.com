defmodule MyAppWeb.PageLive do
  use MyAppWeb, :live_view

  def mount(_params, _session, socket) do
    {:ok, assign(socket, items: [], compact: false)}
  end

  def render(assigns) do
    ~H"""
    <Layouts.app flash={@flash}>
      <.header>
        Tasks
        <:actions>
          <.button variant="primary" phx-click="refresh">
            <.icon name="hero-arrow-path-solid" class="size-4 phx-click-loading:animate-spin" /> Refresh
          </.button>
        </:actions>
      </.header>
      <.task_list items={@items} compact={@compact} />
    </Layouts.app>
    """
  end

  attr :items, :list, required: true
  attr :compact, :boolean, default: false

  def task_list(assigns) do
    ~H"""
    <ul class={["menu bg-base-200 rounded-box w-full", @compact && "menu-sm"]}>
      <li :for={item <- @items} class={if item.done, do: "line-through opacity-60", else: "font-medium"}>
        <span><.icon :if={item.done} name="hero-check-mini" class="text-success" /> {item.title}</span>
      </li>
    </ul>
    """
  end

  def handle_event("refresh", _params, socket) do
    {:noreply, put_flash(socket, :info, "Done")}
  end
end

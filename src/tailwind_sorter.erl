-module(tailwind_sorter).
-moduledoc """
The sort of the classes of templates, as the official Prettier plugin of
Tailwind CSS (`prettier-plugin-tailwindcss`) sorts them, with the design
system of a profile: its input stylesheet (theme, plugins, utilities) and
its version of Tailwind CSS.

A sorter is a process. It runs `tailwind.com --sort-classes` once, as a
NIF run when `tailwind:nif/0` is true, else as a port, and it sends each
request of `sort/2` to that run. More than one process can call one
sorter: it answers the calls in their sequence.

```erlang
{ok, Sorter} = tailwind_sorter:start_link(default),
#{<<"attributes">> := [<<"flex p-4">>]} =
    tailwind_sorter:sort(Sorter, #{<<"attributes">> => [<<"p-4 flex">>]}).
```

A request is a map, and each part is optional:

- `<<"attributes">>`: values of `class` attributes.
- `<<"templates">>`: for each string with interpolations, its fixed
  parts: `"a #{@x} b"` has the parts `<<"a ">>` and `<<" b">>`. A part
  keeps its class at the side of an interpolation.
- `<<"lists">>`: lists of classes.
- `<<"preserveWhitespace">>` and `<<"preserveDuplicates">>`: the options
  of the plugin (default: `false`).

The answer has the same parts, sorted, or `<<"error">>` with a message.
`docs/FORMATTER.md` has more.
""".
-behaviour(gen_server).

-export([start_link/1, start_link/2, sort/2, sort/3, stop/1, sort_args/1]).
-export([init/1, handle_call/3, handle_cast/2, handle_info/2, terminate/2]).

%% The variable that gives the version of Tailwind CSS to tailwind.com.
-define(VERSION_VARIABLE, "TAILWINDCSS_VERSION").

-type request() :: #{binary() => term()}.
-export_type([request/0]).

-doc """
Start a sorter for the profile `Profile` of the configuration of the
application `tailwind`: its `--input`, `--cwd`, `cd` and `env`, and the
version of the configuration. The other arguments of the profile (for
example `--output` and `--watch`) are not for the sort.
""".
-spec start_link(atom()) -> gen_server:start_ret().
start_link(Profile) ->
    start_link(Profile, []).

-doc "Start a sorter, as `start_link/1` does, with the options of `gen_server:start_link/3`.".
-spec start_link(atom(), [gen_server:start_opt()]) -> gen_server:start_ret().
start_link(Profile, Options) when is_atom(Profile) ->
    gen_server:start_link(?MODULE, Profile, Options).

-doc "Sort the classes of a request, with a timeout of 60 seconds.".
-spec sort(gen_server:server_ref(), request()) -> request().
sort(Sorter, Request) ->
    sort(Sorter, Request, 60000).

-doc "Sort the classes of a request.".
-spec sort(gen_server:server_ref(), request(), timeout()) -> request().
sort(Sorter, Request, Timeout) when is_map(Request) ->
    gen_server:call(Sorter, {sort, Request}, Timeout).

-doc "Stop a sorter: the run of the program ends.".
-spec stop(gen_server:server_ref()) -> ok.
stop(Sorter) ->
    gen_server:stop(Sorter).

-doc """
The arguments of a profile for the sort: `--input`, `-i` and `--cwd`
with their values, and `--sort-classes`.
""".
-spec sort_args([string()]) -> [string()].
sort_args(Args) ->
    keep(Args) ++ ["--sort-classes"].

keep([]) ->
    [];
keep([Flag, Value | Rest]) when Flag =:= "--input"; Flag =:= "-i"; Flag =:= "--cwd" ->
    [Flag, Value | keep(Rest)];
keep([Arg | Rest]) ->
    case lists:any(fun(Prefix) -> lists:prefix(Prefix, Arg) end, ["--input=", "-i=", "--cwd="]) of
        true -> [Arg | keep(Rest)];
        false -> keep(Rest)
    end.

%% The state: the run (`{nif, Handle}` or `{port, Port}`), the text of the
%% output after the last end of line, and the callers that wait, in the
%% sequence of their requests.
-doc false.
init(Profile) ->
    #{args := Args, cd := Cd, env := Env0} = tailwind:config_for(Profile),
    Env = maps:to_list(maps:merge(#{?VERSION_VARIABLE => tailwind:configured_version()}, Env0)),
    Argv = sort_args(Args),
    Run =
        case tailwind:nif() of
            true ->
                {nif, tailwind_nif:start(Argv, #{cd => Cd, env => Env})};
            false ->
                {Exe, PortArgs} = tailwind:command(Argv),
                {port,
                    open_port(
                        {spawn_executable, Exe},
                        [{args, PortArgs}, {cd, Cd}, {env, Env}, binary, exit_status, use_stdio, hide]
                    )}
        end,
    {ok, #{run => Run, buffer => <<>>, waiting => queue:new()}}.

-doc false.
handle_call({sort, Request}, From, #{run := Run, waiting := Waiting} = State) ->
    Line = [json:encode(Request), $\n],
    case write(Run, Line) of
        ok -> {noreply, State#{waiting := queue:in(From, Waiting)}};
        closed -> {reply, #{<<"error">> => <<"The sorter has ended.">>}, State}
    end.

-doc false.
handle_cast(_Message, State) ->
    {noreply, State}.

-doc false.
handle_info({Port, {data, Data}}, #{run := {port, Port}} = State) ->
    {noreply, output(Data, State)};
handle_info({Port, {exit_status, Status}}, #{run := {port, Port}} = State) ->
    {stop, {exit_status, Status}, State};
handle_info({tailwind_nif, Id, stdout, Data}, #{run := {nif, {_, Id}}} = State) ->
    {noreply, output(Data, State)};
handle_info({tailwind_nif, Id, stderr, Data}, #{run := {nif, {_, Id}}} = State) ->
    io:put_chars(standard_error, Data),
    {noreply, State};
handle_info({tailwind_nif, Id, exit, Status}, #{run := {nif, {_, Id}}} = State) ->
    {stop, {exit_status, Status}, State};
handle_info(_Message, State) ->
    {noreply, State}.

-doc false.
terminate(_Reason, #{run := Run, waiting := Waiting}) ->
    [gen_server:reply(From, #{<<"error">> => <<"The sorter has ended.">>}) || From <- queue:to_list(Waiting)],
    case Run of
        {nif, Handle} ->
            tailwind_nif:close(Handle);
        {port, Port} ->
            try port_close(Port) of
                true -> ok
            catch
                error:badarg -> ok
            end
    end,
    ok.

write({nif, Handle}, Line) ->
    tailwind_nif:input(Handle, Line);
write({port, Port}, Line) ->
    try port_command(Port, Line) of
        true -> ok
    catch
        error:badarg -> closed
    end.

%% Each line of the output is the answer to the request that waits the
%% longest.
output(Data, #{buffer := Buffer} = State) ->
    case binary:split(<<Buffer/binary, Data/binary>>, <<"\n">>) of
        [Rest] ->
            State#{buffer := Rest};
        [Line, Rest] ->
            #{waiting := Waiting} = State,
            case queue:out(Waiting) of
                {{value, From}, Waiting1} ->
                    gen_server:reply(From, json:decode(Line)),
                    output(<<>>, State#{buffer := Rest, waiting := Waiting1});
                {empty, _} ->
                    output(<<>>, State#{buffer := Rest})
            end
    end.

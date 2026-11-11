------------------------------- MODULE Watch -------------------------------
(* The watch of tailwind.com (--watch): native/tailwind/src/cli/mod.rs.

   The program polls. At each tick it compares a stamp (the time and the
   size) of the input stylesheet with the stamp that it recorded. When
   they are different it makes a full build: a new compiler from the
   input. If not, it scans the source files and builds when they have new
   candidates. A person edits the input and the source files at any time,
   also in the middle of a tick.

   The model checks these properties:

     - NoBrokenOutput: the output never comes from an input with an error.
     - OutputFromSeenInput: the output comes from a version of the input
       that was on the disk.
     - ErrorTwice: one version of the input gives its error two times at
       most. (Two times: an edit between the stamp and the read.)
     - Converges: when the edits stop and the input has no error, the
       output becomes the build of the last input with all the candidates
       of the source files.

   StampBeforeRead = FALSE is the wrong order: the program reads the
   input and then records the stamp. A change between the two steps is
   then lost, and Converges fails. The first version of the code had this
   order for the files of a new compiler. *)
EXTENDS Naturals, FiniteSets

CONSTANTS
    MaxVersion,       \* the number of edits of the input
    Broken,           \* the versions of the input that have an error
    Candidates,       \* the candidates that the source files can get
    StampBeforeRead   \* TRUE: the program records the stamp before the read

ASSUME MaxVersion \in Nat \ {0} /\ Broken \subseteq 1..MaxVersion

VARIABLES
    input,        \* the version of the input on the disk, from 1
    sources,      \* the candidates in the source files on the disk
    stamp,        \* the stamp of the input that the program recorded
    readVersion,  \* the version that the program read in this tick
    compiler,     \* the input version of the compiler; 0 before the first
    known,        \* the candidates that the scanner of the compiler knows
    output,       \* the last output: <<input version, candidates>>
    errors,       \* the versions for which the program printed an error
    reported,     \* the number of error messages for each version
    pc            \* the step of the program

vars == <<input, sources, stamp, readVersion, compiler, known, output, errors, reported, pc>>

None == <<0, {}>>

Init ==
    /\ input = 1
    /\ sources = {}
    /\ stamp = 0
    /\ readVersion = 0
    /\ compiler = 0
    /\ known = {}
    /\ output = None
    /\ errors = {}
    /\ reported = [v \in 0..MaxVersion |-> 0]
    /\ pc = "start"

(* The person. *)
EditInput ==
    /\ input < MaxVersion
    /\ input' = input + 1
    /\ UNCHANGED <<sources, stamp, readVersion, compiler, known, output, errors, reported, pc>>

EditSource ==
    /\ \E c \in Candidates \ sources : sources' = sources \cup {c}
    /\ UNCHANGED <<input, stamp, readVersion, compiler, known, output, errors, reported, pc>>

(* The first build: record the stamp and read the input. *)
Start ==
    /\ pc = "start"
    /\ stamp' = IF StampBeforeRead THEN input ELSE stamp
    /\ pc' = "read"
    /\ UNCHANGED <<input, sources, readVersion, compiler, known, output, errors, reported>>

(* A tick: compare the stamps. *)
Tick ==
    /\ pc = "idle"
    /\ IF stamp # input
          THEN /\ pc' = "read"
               /\ stamp' = IF StampBeforeRead THEN input ELSE stamp
          ELSE /\ pc' = "scan"
               /\ stamp' = stamp
    /\ UNCHANGED <<input, sources, readVersion, compiler, known, output, errors, reported>>

(* Read the input. With the wrong order, the stamp is recorded only
   after the read, in the step Stamp. *)
Read ==
    /\ pc = "read"
    /\ readVersion' = input
    /\ pc' = IF StampBeforeRead THEN "compile" ELSE "stamp"
    /\ UNCHANGED <<input, sources, stamp, compiler, known, output, errors, reported>>

Stamp ==
    /\ pc = "stamp"
    /\ stamp' = input
    /\ pc' = "compile"
    /\ UNCHANGED <<input, sources, readVersion, compiler, known, output, errors, reported>>

(* Make the compiler. An input with an error keeps the old compiler. *)
Compile ==
    /\ pc = "compile"
    /\ IF readVersion \in Broken
          THEN /\ errors' = errors \cup {readVersion}
               /\ reported' = [reported EXCEPT ![readVersion] = @ + 1]
               \* An error in the first build ends the program.
               /\ pc' = IF compiler = 0 THEN "exit" ELSE "idle"
               /\ UNCHANGED <<compiler, known>>
          ELSE /\ compiler' = readVersion
               /\ known' = {}
               /\ pc' = "scan"
               /\ UNCHANGED <<errors, reported>>
    /\ UNCHANGED <<input, sources, stamp, readVersion, output>>

(* Scan the source files, build and write. *)
Scan ==
    /\ pc = "scan"
    /\ known' = known \cup sources
    /\ output' = <<compiler, known \cup sources>>
    /\ pc' = "idle"
    /\ UNCHANGED <<input, sources, stamp, readVersion, compiler, errors, reported>>

Program == Start \/ Tick \/ Read \/ Stamp \/ Compile \/ Scan

Next == EditInput \/ EditSource \/ Program

Spec == Init /\ [][Next]_vars /\ WF_vars(Program)

-----------------------------------------------------------------------------

TypeOK ==
    /\ input \in 1..MaxVersion
    /\ sources \subseteq Candidates
    /\ stamp \in 0..MaxVersion
    /\ readVersion \in 0..MaxVersion
    /\ compiler \in 0..MaxVersion
    /\ known \subseteq Candidates
    /\ output[1] \in 0..MaxVersion /\ output[2] \subseteq Candidates
    /\ pc \in {"start", "idle", "read", "stamp", "compile", "scan", "exit"}

NoBrokenOutput == output[1] \notin Broken

(* The scanner of a compiler only adds candidates, and the output has
   all of them. *)
OutputHasKnown == output # None => (output[2] = known \/ pc = "scan" \/ output[1] # compiler)

OutputFromSeenInput == output[1] <= input /\ output[2] \subseteq sources

ErrorTwice == \A v \in 0..MaxVersion : reported[v] <= 2

(* The edits stop, and the last input has no error: the output becomes
   the build of this input with all the candidates. *)
Stable == input = MaxVersion /\ sources = Candidates

Converges ==
    (MaxVersion \notin Broken) =>
        <>[]((Stable /\ pc # "exit") => output = <<MaxVersion, Candidates>>)

=============================================================================

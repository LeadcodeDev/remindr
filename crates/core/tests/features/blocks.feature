Feature: Blocks

  @spec-v1-1
  Scenario: Typing in an empty page creates one text block
    Given a page "Notes"
    When I type "Hello" in the empty page "Notes"
    Then the command succeeds
    And the children of "Notes" read:
      | "Hello" |
    And the first child of "Notes" is a text block

  @spec-v1-2
  Scenario: Splitting a block at an offset gives two blocks in order
    Given a page "Notes"
    And the page "Notes" has the blocks:
      | name | text        |
      | a    | Hello world |
    When I split "a" at offset 5
    Then the command succeeds
    And the children of "Notes" read:
      | "Hello"  |
      | " world" |

  @spec-v1-3
  Scenario: Enter on an empty bulleted list item turns it into a text block
    Given a page "Notes"
    And the page "Notes" has the blocks:
      | name | kind     | text |
      | a    | bulleted |      |
    When I split "a" at offset 0
    Then the command succeeds
    And block "a" is a text block
    And the children of "Notes" read:
      | "" |

  @spec-v1-4
  Scenario: Backspace at offset 0 merges a block into the previous one
    Given a page "Notes"
    And the page "Notes" has the blocks:
      | name | text |
      | a    | ab   |
      | b    | cd   |
    When I press Backspace at the start of "b"
    Then the command succeeds
    And the children of "Notes" read:
      | "abcd" |
    And block "b" is deleted

  @spec-v1-5
  Scenario: Deleting a range across two blocks leaves one block
    Given a page "Notes"
    And the page "Notes" has the blocks:
      | name | text  |
      | a    | Hello |
      | b    | world |
    When I delete from offset 2 of "a" to offset 3 of "b"
    Then the command succeeds
    And the children of "Notes" read:
      | "Held" |

  @spec-v1-8
  Scenario: Indent nests under the previous sibling and outdent places it after its former parent
    Given a page "Notes"
    And the page "Notes" has the blocks:
      | name | text  |
      | a    | one   |
      | b    | two   |
      | c    | three |
    When I indent "b"
    Then the command succeeds
    And the children of "Notes" read:
      | "one"   |
      | "three" |
    And the children of "a" read:
      | "two" |
    When I outdent "b"
    Then the command succeeds
    And the children of "Notes" read:
      | "one"   |
      | "two"   |
      | "three" |

  @spec-v1-9
  Scenario: Indent on a first child is rejected
    Given a page "Notes"
    And the page "Notes" has the blocks:
      | name | text |
      | a    | one  |
      | b    | two  |
    When I indent "a"
    Then the command is rejected
    And the workspace is unchanged

  @spec-v1-9
  Scenario: Indent under a heading is rejected
    Given a page "Notes"
    And the page "Notes" has the blocks:
      | name | kind    | text  |
      | h    | heading | Title |
      | b    | text    | two   |
    When I indent "b"
    Then the command is rejected
    And the workspace is unchanged

  @spec-v1-11
  Scenario: Moving a block with two children moves the three blocks in order
    Given a page "Notes"
    And the page "Notes" has the blocks:
      | name | text  | parent |
      | a    | alpha |        |
      | k1   | one   | a      |
      | k2   | two   | a      |
      | b    | beta  |        |
    When I move "a" under "b"
    Then the command succeeds
    And the children of "Notes" read:
      | "beta" |
    And the children of "b" read:
      | "alpha" |
    And the children of "a" read:
      | "one" |
      | "two" |

  @spec-v1-12
  Scenario: Moving a block under one of its descendants is rejected
    Given a page "Notes"
    And the page "Notes" has the blocks:
      | name | text  | parent |
      | a    | alpha |        |
      | k    | kid   | a      |
    When I move "a" under "k"
    Then the command is rejected
    And the workspace is unchanged

  @spec-v1-13
  Scenario: Typing within one second is a single undo step
    Given a page "Notes"
    And the page "Notes" has the blocks:
      | name | text |
      | a    |      |
    When I type "a" into "a"
    And 300 ms pass
    And I type "ab" into "a"
    And 300 ms pass
    And I type "abc" into "a"
    And I undo
    Then block "a" has text ""
    When I redo
    Then block "a" has text "abc"

  @spec-v1-14
  Scenario: Deleting a page hides its subtree and undo restores it
    Given a page "Notes"
    And the page "Notes" has the blocks:
      | name | text  | parent |
      | a    | alpha |        |
      | b    | beta  | a      |
    When I delete the page "Notes"
    Then the command succeeds
    And the workspace has no pages
    And block "a" is deleted
    And block "b" is deleted
    When I undo
    Then the page "Notes" is listed among the pages
    And block "a" is alive
    And block "b" is alive

  @spec-v1-35
  Scenario: A commit failure leaves the workspace unchanged
    Given a page "Notes"
    And the page "Notes" has the blocks:
      | name | text  |
      | a    | hello |
    And the next commit fails
    When I type "lost" into "a"
    Then the command is rejected
    And the workspace is unchanged
    And block "a" has text "hello"

@databases
Feature: Databases

  @spec-v1-17
  Scenario: A new database has a title property and is not a page
    Given an empty workspace
    When I create a database from the sidebar
    Then the command succeeds
    And the database "Untitled" is listed among the databases
    And the database "Untitled" has the properties:
      | name | kind  |
      | Name | title |
    And the workspace has no pages

  @spec-v1-18
  Scenario: Inserting a database in a page creates a view block with one table view
    Given a page "Notes"
    When I insert a new database in the page "Notes" as "Tasks block"
    Then the command succeeds
    And the page "Notes" contains the database view block "Tasks block"
    And the block "Tasks block" shows the database "Untitled"
    And the block "Tasks block" has exactly one view named "Table" of kind table
    And the database "Untitled" is listed among the databases

  @spec-v1-20
  Scenario: Rows only come from add row
    Given a page "Notes"
    And a database "Tasks" shown in the page "Notes" as "Tasks block"
    When I create a page under the database "Tasks"
    Then the command is rejected
    And the workspace is unchanged
    When I add a row "Write spec" to the database "Tasks"
    Then the command succeeds
    And the rows of the database "Tasks" read:
      | Write spec |

  @spec-v1-21
  Scenario: Values of every kind are stored and a wrong kind is rejected
    Given a page "Notes"
    And a database "Tasks" shown in the page "Notes" as "Tasks block"
    And the database "Tasks" has the properties:
      | name   | kind         |
      | Memo   | text         |
      | Amount | number       |
      | Status | select       |
      | Tags   | multi-select |
      | Due    | date         |
      | Done   | checkbox     |
      | Link   | url          |
    And the property "Status" has the options "Todo, Doing, Done"
    And the property "Tags" has the options "Red, Blue"
    And the database "Tasks" has the rows:
      | name |
      | a    |
    When I set the values of the row "a":
      | property | value                |
      | Memo     | hello                |
      | Amount   | 42.5                 |
      | Status   | Doing                |
      | Tags     | Red, Blue            |
      | Due      | 2026-10-23           |
      | Done     | true                 |
      | Link     | https://example.test |
    Then the command succeeds
    And the row "a" has the values:
      | property | value                |
      | Memo     | hello                |
      | Amount   | 42.5                 |
      | Status   | Doing                |
      | Tags     | Red, Blue            |
      | Due      | 2026-10-23           |
      | Done     | true                 |
      | Link     | https://example.test |
    When I set the text value "abc" on the property "Amount" of the row "a"
    Then the command is rejected
    And the workspace is unchanged
    And the row "a" has the value "42.5" for "Amount"

  @spec-v1-22
  Scenario: Removing a select option clears the values that reference it in one undo step
    Given a page "Notes"
    And a database "Tasks" shown in the page "Notes" as "Tasks block"
    And the database "Tasks" has the properties:
      | name   | kind   |
      | Status | select |
    And the property "Status" has the options "Todo, Doing, Done"
    And the database "Tasks" has the rows:
      | name | Status |
      | a    | Done   |
      | b    | Todo   |
    When I remove the option "Done" from the property "Status"
    Then the command succeeds
    And the row "a" has no value for "Status"
    And the row "b" has the value "Todo" for "Status"
    And the property "Status" has the options "Todo, Doing"
    When I undo
    Then the row "a" has the value "Done" for "Status"
    And the property "Status" has the options "Todo, Doing, Done"

  @spec-v1-23
  Scenario: Removing a property removes its values and what references it
    Given a page "Notes"
    And a database "Tasks" shown in the page "Notes" as "Tasks block"
    And the database "Tasks" has the properties:
      | name   | kind   |
      | Status | select |
      | Amount | number |
    And the property "Status" has the options "Todo, Done"
    And the database "Tasks" has the rows:
      | name | Status | Amount |
      | a    | Done   | 1      |
      | b    | Todo   | 2      |
    And a board view "Board" grouped by "Status" in the block "Tasks block"
    And I filter the view "Tasks block" to rows whose "Status" is "Done"
    And I sort the view "Tasks block" by:
      | property | direction  |
      | Status   | ascending  |
      | Amount   | descending |
    When I remove the property "Status" from the database "Tasks"
    Then the command succeeds
    And the row "a" has no stored value for "Status"
    And the row "b" has no stored value for "Status"
    And the view "Tasks block" has 0 filters
    And the view "Tasks block" has 1 sorts
    And the view "Board" is a table view
    And the view "Tasks block" shows the rows:
      | b |
      | a |
    When I undo
    Then the row "a" has the value "Done" for "Status"
    And the view "Board" is a board view
    And the view "Tasks block" has 1 filters
    And the view "Tasks block" has 2 sorts

  @spec-v1-24
  Scenario: A select filter returns only the matching rows
    Given a page "Notes"
    And a database "Tasks" shown in the page "Notes" as "Tasks block"
    And the database "Tasks" has the properties:
      | name   | kind   |
      | Status | select |
    And the property "Status" has the options "Todo, Doing, Done"
    And the database "Tasks" has the rows:
      | name | Status |
      | a    | Done   |
      | b    | Todo   |
      | c    | Done   |
      | d    |        |
    When I filter the view "Tasks block" to rows whose "Status" is "Done"
    Then the command succeeds
    And the view "Tasks block" shows the rows:
      | a |
      | c |

  @spec-v1-25
  Scenario: Sorting by number descending then title ascending puts empty numbers last
    Given a page "Notes"
    And a database "Tasks" shown in the page "Notes" as "Tasks block"
    And the database "Tasks" has the properties:
      | name   | kind   |
      | Amount | number |
    And the database "Tasks" has the rows:
      | name | Amount |
      | b    | 5      |
      | a    | 5      |
      | c    | 10     |
      | d    |        |
    When I sort the view "Tasks block" by:
      | property | direction  |
      | Amount   | descending |
      | Name     | ascending  |
    Then the command succeeds
    And the view "Tasks block" shows the rows:
      | c |
      | a |
      | b |
      | d |

  @spec-v1-26
  Scenario: A board has one group per option plus an empty group and moving a card sets its value
    Given a page "Notes"
    And a database "Tasks" shown in the page "Notes" as "Tasks block"
    And the database "Tasks" has the properties:
      | name   | kind   |
      | Status | select |
    And the property "Status" has the options "Todo, Doing, Done"
    And the database "Tasks" has the rows:
      | name | Status |
      | a    |        |
      | b    |        |
      | c    | Todo   |
    And a board view "Board" grouped by "Status" in the block "Tasks block"
    Then the board "Board" has the groups:
      | group     | rows |
      | No Status | a, b |
      | Todo      | c    |
      | Doing     |      |
      | Done      |      |
    When I move the card "a" to the group "Done" of the board "Board"
    Then the command succeeds
    And the row "a" has the value "Done" for "Status"
    And the board "Board" has the groups:
      | group     | rows |
      | No Status | b    |
      | Todo      | c    |
      | Doing     |      |
      | Done      | a    |

  @spec-v1-27
  Scenario: Two views keep separate manual orders
    Given a page "Notes"
    And a database "Tasks" shown in the page "Notes" as "Tasks block"
    And the database "Tasks" has the rows:
      | name |
      | a    |
      | b    |
      | c    |
    And a table view "Second" in the block "Tasks block"
    When I move the row "c" before "a" in the view "Tasks block"
    Then the command succeeds
    And the view "Tasks block" shows the rows:
      | c |
      | a |
      | b |
    And the view "Second" shows the rows:
      | a |
      | b |
      | c |

  @spec-v1-28
  Scenario: Manual reorder in a view with sorts is rejected
    Given a page "Notes"
    And a database "Tasks" shown in the page "Notes" as "Tasks block"
    And the database "Tasks" has the rows:
      | name |
      | a    |
      | b    |
    And I sort the view "Tasks block" by:
      | property | direction |
      | Name     | ascending |
    When I move the row "b" before "a" in the view "Tasks block"
    Then the command is rejected
    And the workspace is unchanged
    And the view "Tasks block" shows the rows:
      | a |
      | b |

  @spec-v1-29
  Scenario: Two view blocks of one database keep separate filters and share values
    Given a page "Notes"
    And a database "Tasks" shown in the page "Notes" as "First block"
    And the database "Tasks" has the properties:
      | name   | kind   |
      | Status | select |
    And the property "Status" has the options "Todo, Done"
    And the database "Tasks" has the rows:
      | name | Status |
      | a    | Done   |
      | b    | Todo   |
    And I insert another view block of the database "Tasks" after "First block" as "Second block"
    When I filter the view "First block" to rows whose "Status" is "Done"
    Then the view "First block" shows the rows:
      | a |
    And the view "Second block" shows the rows:
      | a |
      | b |
    When I set the value "Done" on the property "Status" of the row "b"
    Then the command succeeds
    And the view "First block" shows the rows:
      | a |
      | b |
    And the view "Second block" shows the rows:
      | a |
      | b |

  @spec-v1-30
  Scenario: Deleting a database view block leaves the database and its rows unchanged
    Given a page "Notes"
    And a database "Tasks" shown in the page "Notes" as "Tasks block"
    And the database "Tasks" has the rows:
      | name |
      | a    |
      | b    |
    When I delete the view block "Tasks block"
    Then the command succeeds
    And block "Tasks block" is deleted
    And the database "Tasks" is listed among the databases
    And the rows of the database "Tasks" read:
      | a |
      | b |

  @spec-v1-31
  Scenario: Deleting a database leaves a placeholder and undo restores it with its rows
    Given a page "Notes"
    And a database "Tasks" shown in the page "Notes" as "Tasks block"
    And the database "Tasks" has the rows:
      | name |
      | a    |
      | b    |
    When I delete the database "Tasks"
    Then the command succeeds
    And the database "Tasks" is not listed among the databases
    And the block "Tasks block" shows a placeholder
    And the view "Tasks block" shows no rows
    When I try to add a row to the database "Tasks"
    Then the command is rejected
    And the workspace is unchanged
    When I undo
    Then the database "Tasks" is listed among the databases
    And the block "Tasks block" shows the database "Tasks"
    And the rows of the database "Tasks" read:
      | a |
      | b |
    And the view "Tasks block" shows the rows:
      | a |
      | b |

  @spec-v1-32
  Scenario: Removing the last view of a database view block is rejected
    Given a page "Notes"
    And a database "Tasks" shown in the page "Notes" as "Tasks block"
    When I remove the view "Tasks block"
    Then the command is rejected
    And the workspace is unchanged

  @spec-v1-33
  Scenario: Changing a text property to number keeps numbers, clears the rest, and undo restores both
    Given a page "Notes"
    And a database "Tasks" shown in the page "Notes" as "Tasks block"
    And the database "Tasks" has the properties:
      | name     | kind |
      | Quantity | text |
    And the database "Tasks" has the rows:
      | name | Quantity |
      | a    | 42       |
      | b    | abc      |
    When I change the type of the property "Quantity" to number
    Then the command succeeds
    And the property "Quantity" is a number property
    And the row "a" has the value "42" for "Quantity"
    And the row "b" has no value for "Quantity"
    When I undo
    Then the property "Quantity" is a text property
    And the row "a" has the value "42" for "Quantity"
    And the row "b" has the value "abc" for "Quantity"

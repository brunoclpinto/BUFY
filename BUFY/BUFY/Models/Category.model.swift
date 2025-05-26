//
//  Category.model.swift
//  BUFY
//
//  Created by Bruno Pinto on 14/05/2025.
//

/// Represents the subset where the movement exists for improved metrics evaluation and better understanding of value flow.
/// Set of emojis and name combination are the identifiers of this object.
/// They are both expected to not be empty.
struct Category: Hashable, Codable {
  func hash(into hasher: inout Hasher) {
    hasher.combine(name.lowercased())
    hasher.combine(emoji.lowercased())
  }
  
  static func == (lhs: Category, rhs: Category) -> Bool {
    lhs.name.lowercased() == rhs.name.lowercased() &&
    lhs.emoji.lowercased() == rhs.emoji.lowercased()
  }
  
  private(set) var emoji: String
  private(set) var name: String
  var description: String = ""
  
  init(
    emoji: String,
    name: String,
    description: String = ""
  ) throws {
    try Self.isValidEmoji(emoji)
    try Self.isValidName(name)
    
    self.emoji = emoji
    self.name = name
    self.description = description
  }
  
  mutating func setEmoji(_ emoji: String) throws {
    try Self.isValidEmoji(emoji)
    self.emoji = emoji
  }
  
  mutating func setName(_ name: String) throws {
    try Self.isValidName(name)
    self.name = name
  }
}

/// Validation and error handling
extension Category {
  private static func isValidEmoji(_ emoji: String) throws {
    guard !emoji.isEmpty else {
      throw CustomError.invalidArgument(
        argument: "emoji",
        currentValue: emoji,
        expected: "Not empty",
        classType: Category.self
      )
    }
    guard emoji.containsOnlyEmoji else {
      throw CustomError.invalidArgument(
        argument: "emoji",
        currentValue: emoji,
        expected: "Only contain emojis",
        classType: Category.self
      )
    }
  }
  
  private static func isValidName(_ name: String) throws {
    guard !name.isEmpty else {
      throw CustomError.invalidArgument(
        argument: "Name",
        currentValue: name,
        expected: "Not empty",
        classType: Entity.self
      )
    }
  }
}

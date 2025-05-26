//
//  Movement.model.swift
//  BUFY
//
//  Created by Bruno Pinto on 26/05/2025.
//

import Foundation

struct Movement<From: EntityProtocol, To: EntityProtocol>: Hashable, Codable {
  func hash(into hasher: inout Hasher) {
    hasher.combine(title.lowercased())
  }
  
  static func == (lhs: Movement<From, To>, rhs: Movement<From, To>) -> Bool {
    lhs.title.lowercased() == rhs.title.lowercased()
  }
  
  private(set) var title: String
  var description: String = ""
  var from: From
  var to: To
  private(set) var value: Double
  var repeats: Repeats?
  var isDebit: Bool
  
  struct Repeats: Hashable, Codable {
    var start: Date
    var repeats: DateComponents
    var ends: Date
  }
  
  init(
    title: String,
    description: String = "",
    from: From,
    to: To,
    value: Double = 0,
    repeats: Repeats? = nil,
    isDebit: Bool = false
  ) throws {
    try Self.isValidTitle(title)
    try Self.isValidValue(value)
    
    self.title = title
    self.description = description
    self.from = from
    self.to = to
    self.value = value
    self.repeats = repeats
    self.isDebit = isDebit
  }
  
  mutating func setTitle(title: String) throws {
    try Self.isValidTitle(title)
    
    self.title = title
  }
  
  mutating func setValue(value: Double) throws {
    try Self.isValidValue(value)
    self.value = value
  }
}

extension Movement {
  private static func isValidTitle(_ title: String) throws {
    guard !title.isEmpty else {
      throw CustomError.invalidArgument(
        argument: "Title",
        currentValue: title,
        expected: "Not empty",
        classType: Movement.self
      )
    }
  }
  
  private static func isValidValue(_ value: Double) throws {
    guard value >= 0 else {
      throw CustomError.invalidArgument(
        argument: "Value",
        currentValue: "\(value)",
        expected: ">= zero",
        classType: Movement.self
      )
    }
  }
}

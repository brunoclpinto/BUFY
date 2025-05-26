//
//  File.swift
//  BUFY
//
//  Created by Bruno Pinto on 16/11/2024.
//

import Foundation

struct Transaction<From: EntityProtocol, To: EntityProtocol>: Hashable, Codable{
  let movement: Movement<From, To>
  var expected: Date
  var performed: Date?
  private(set) var value: Double
  
  init(
    movement: Movement<From, To>,
    expected: Date,
    performed: Date? = nil,
    value: Double
  ) throws {
    try Self.isValidValue(value)
    
    self.movement = movement
    self.expected = expected
    self.performed = performed
    self.value = value
  }
  
  mutating func setValue(_ value: Double) throws {
    try Self.isValidValue(value)
    self.value = value
  }
}

extension Transaction {
  private static func isValidValue(_ value: Double) throws {
    guard value >= 0 else {
      throw CustomError.invalidArgument(
        argument: "Value",
        currentValue: "\(value)",
        expected: ">= zero",
        classType: Transaction.self
      )
    }
  }
}

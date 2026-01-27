from toolapi._core import sum_as_string


def test_sum_as_string():
    result = sum_as_string(2, 3)
    assert result == "5", f"Expected '5', got '{result}'"
    print(f"sum_as_string(2, 3) = {result}")


if __name__ == "__main__":
    test_sum_as_string()
    print("All tests passed.")

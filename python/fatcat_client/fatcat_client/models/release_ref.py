# coding: utf-8

"""
    fatcat

    A scalable, versioned, API-oriented catalog of bibliographic entities and file metadata  # noqa: E501

    OpenAPI spec version: 0.1.0
    
    Generated by: https://github.com/swagger-api/swagger-codegen.git
"""


import pprint
import re  # noqa: F401

import six


class ReleaseRef(object):
    """NOTE: This class is auto generated by the swagger code generator program.

    Do not edit the class manually.
    """

    """
    Attributes:
      swagger_types (dict): The key is attribute name
                            and the value is attribute type.
      attribute_map (dict): The key is attribute name
                            and the value is json key in definition.
    """
    swagger_types = {
        'index': 'int',
        'target_release_id': 'str',
        'stub': 'str'
    }

    attribute_map = {
        'index': 'index',
        'target_release_id': 'target_release_id',
        'stub': 'stub'
    }

    def __init__(self, index=None, target_release_id=None, stub=None):  # noqa: E501
        """ReleaseRef - a model defined in Swagger"""  # noqa: E501

        self._index = None
        self._target_release_id = None
        self._stub = None
        self.discriminator = None

        if index is not None:
            self.index = index
        if target_release_id is not None:
            self.target_release_id = target_release_id
        if stub is not None:
            self.stub = stub

    @property
    def index(self):
        """Gets the index of this ReleaseRef.  # noqa: E501


        :return: The index of this ReleaseRef.  # noqa: E501
        :rtype: int
        """
        return self._index

    @index.setter
    def index(self, index):
        """Sets the index of this ReleaseRef.


        :param index: The index of this ReleaseRef.  # noqa: E501
        :type: int
        """

        self._index = index

    @property
    def target_release_id(self):
        """Gets the target_release_id of this ReleaseRef.  # noqa: E501


        :return: The target_release_id of this ReleaseRef.  # noqa: E501
        :rtype: str
        """
        return self._target_release_id

    @target_release_id.setter
    def target_release_id(self, target_release_id):
        """Sets the target_release_id of this ReleaseRef.


        :param target_release_id: The target_release_id of this ReleaseRef.  # noqa: E501
        :type: str
        """

        self._target_release_id = target_release_id

    @property
    def stub(self):
        """Gets the stub of this ReleaseRef.  # noqa: E501


        :return: The stub of this ReleaseRef.  # noqa: E501
        :rtype: str
        """
        return self._stub

    @stub.setter
    def stub(self, stub):
        """Sets the stub of this ReleaseRef.


        :param stub: The stub of this ReleaseRef.  # noqa: E501
        :type: str
        """

        self._stub = stub

    def to_dict(self):
        """Returns the model properties as a dict"""
        result = {}

        for attr, _ in six.iteritems(self.swagger_types):
            value = getattr(self, attr)
            if isinstance(value, list):
                result[attr] = list(map(
                    lambda x: x.to_dict() if hasattr(x, "to_dict") else x,
                    value
                ))
            elif hasattr(value, "to_dict"):
                result[attr] = value.to_dict()
            elif isinstance(value, dict):
                result[attr] = dict(map(
                    lambda item: (item[0], item[1].to_dict())
                    if hasattr(item[1], "to_dict") else item,
                    value.items()
                ))
            else:
                result[attr] = value

        return result

    def to_str(self):
        """Returns the string representation of the model"""
        return pprint.pformat(self.to_dict())

    def __repr__(self):
        """For `print` and `pprint`"""
        return self.to_str()

    def __eq__(self, other):
        """Returns true if both objects are equal"""
        if not isinstance(other, ReleaseRef):
            return False

        return self.__dict__ == other.__dict__

    def __ne__(self, other):
        """Returns true if both objects are not equal"""
        return not self == other
